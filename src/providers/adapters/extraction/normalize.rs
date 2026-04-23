use std::collections::HashMap;

use anyhow::{Context, Result};
use memo_engine::ExtractionResult;
use serde::Deserialize;

const MIN_EXTRACTION_CONFIDENCE: f32 = 0.5;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ExtractionCleanupOptions {
    pub min_confidence: f32,
    pub normalize_predicates: bool,
}

impl Default for ExtractionCleanupOptions {
    fn default() -> Self {
        Self {
            min_confidence: MIN_EXTRACTION_CONFIDENCE,
            normalize_predicates: true,
        }
    }
}

#[derive(Deserialize)]
struct ExtractionEnvelope {
    #[serde(default)]
    result: Option<ExtractionResult>,
    #[serde(default)]
    entities: Vec<memo_engine::ExtractedEntity>,
    #[serde(default)]
    facts: Vec<memo_engine::ExtractedFact>,
}

pub(crate) fn parse_extraction_response_with_options(
    content: &str,
    options: ExtractionCleanupOptions,
) -> Result<ExtractionResult> {
    let json = extract_json_object(content);
    if let Ok(result) = serde_json::from_str::<ExtractionResult>(json) {
        return Ok(normalize_extraction_result(result, options));
    }

    let envelope: ExtractionEnvelope =
        serde_json::from_str(json).context("failed to parse extraction JSON response")?;
    let result = envelope.result.unwrap_or(ExtractionResult {
        entities: envelope.entities,
        facts: envelope.facts,
    });
    Ok(normalize_extraction_result(result, options))
}

fn extract_json_object(content: &str) -> &str {
    let trimmed = content.trim();
    if let Some(stripped) = trimmed.strip_prefix("```json") {
        return extract_json_slice(stripped.trim().trim_end_matches("```").trim());
    }
    if let Some(stripped) = trimmed.strip_prefix("```") {
        return extract_json_slice(stripped.trim().trim_end_matches("```").trim());
    }
    extract_json_slice(trimmed)
}

fn extract_json_slice(content: &str) -> &str {
    let Some(start) = content.find('{') else {
        return content;
    };

    let mut depth = 0_usize;
    let mut in_string = false;
    let mut escaped = false;

    for (offset, ch) in content[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
                continue;
            }
            match ch {
                '\\' => escaped = true,
                '"' => in_string = false,
                _ => {}
            }
            continue;
        }

        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    let end = start + offset + ch.len_utf8();
                    return &content[start..end];
                }
            }
            _ => {}
        }
    }

    &content[start..]
}

fn normalize_extraction_result(
    mut result: ExtractionResult,
    options: ExtractionCleanupOptions,
) -> ExtractionResult {
    let mut entity_alias_map = HashMap::<String, String>::new();
    let mut entities = HashMap::<String, memo_engine::ExtractedEntity>::new();

    for mut entity in result.entities.drain(..) {
        entity.name = normalize_name(&entity.name);
        entity.entity_type = entity.entity_type.trim().to_string();
        entity.aliases = entity
            .aliases
            .into_iter()
            .map(|alias| normalize_name(&alias))
            .filter(|alias| !alias.is_empty())
            .fold(Vec::new(), |mut aliases, alias| {
                if !aliases.contains(&alias) {
                    aliases.push(alias);
                }
                aliases
            });
        entity.confidence = entity.confidence.clamp(0.0, 1.0);

        if entity.name.is_empty()
            || entity.entity_type.is_empty()
            || entity.confidence < options.min_confidence
        {
            continue;
        }

        let key = normalize_lookup(&entity.name);
        let merged = entities
            .entry(key)
            .or_insert_with(|| memo_engine::ExtractedEntity {
                entity_type: entity.entity_type.clone(),
                name: entity.name.clone(),
                aliases: Vec::new(),
                confidence: entity.confidence,
            });

        if merged.name.len() < entity.name.len() {
            merged.name = entity.name.clone();
        }
        if merged.entity_type == "unknown" && entity.entity_type != "unknown" {
            merged.entity_type = entity.entity_type.clone();
        }
        merged.confidence = merged.confidence.max(entity.confidence);
        for alias in entity.aliases {
            if alias != merged.name && !merged.aliases.contains(&alias) {
                merged.aliases.push(alias);
            }
        }
    }

    let mut normalized_entities: Vec<memo_engine::ExtractedEntity> =
        entities.into_values().collect();
    normalized_entities.sort_by(|left, right| left.name.cmp(&right.name));

    for entity in &normalized_entities {
        entity_alias_map.insert(normalize_lookup(&entity.name), entity.name.clone());
        for alias in &entity.aliases {
            entity_alias_map.insert(normalize_lookup(alias), entity.name.clone());
        }
    }

    let mut facts = HashMap::<String, memo_engine::ExtractedFact>::new();
    for mut fact in result.facts.drain(..) {
        fact.subject = canonicalize_fact_end(&fact.subject, &entity_alias_map);
        fact.predicate = if options.normalize_predicates {
            normalize_predicate(&fact.predicate)
        } else {
            fact.predicate.trim().to_string()
        };
        fact.object = canonicalize_fact_end(&fact.object, &entity_alias_map);
        fact.confidence = fact.confidence.clamp(0.0, 1.0);

        if fact.subject.is_empty()
            || fact.predicate.is_empty()
            || fact.object.is_empty()
            || fact.confidence < options.min_confidence
        {
            continue;
        }

        let key = format!(
            "{}|{}|{}",
            normalize_lookup(&fact.subject),
            fact.predicate,
            normalize_lookup(&fact.object)
        );
        let merged = facts
            .entry(key)
            .or_insert_with(|| memo_engine::ExtractedFact {
                subject: fact.subject.clone(),
                predicate: fact.predicate.clone(),
                object: fact.object.clone(),
                confidence: fact.confidence,
            });
        merged.confidence = merged.confidence.max(fact.confidence);
        if merged.subject.len() < fact.subject.len() {
            merged.subject = fact.subject.clone();
        }
        if merged.object.len() < fact.object.len() {
            merged.object = fact.object.clone();
        }
    }

    let mut normalized_facts: Vec<memo_engine::ExtractedFact> = facts.into_values().collect();
    normalized_facts.sort_by(|left, right| {
        left.subject
            .cmp(&right.subject)
            .then(left.predicate.cmp(&right.predicate))
            .then(left.object.cmp(&right.object))
    });

    result.entities = normalized_entities;
    result.facts = normalized_facts;
    result
}

fn normalize_name(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return String::new();
    }
    let mut chars = trimmed.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    let mut normalized = String::new();
    normalized.extend(first.to_uppercase());
    normalized.push_str(chars.as_str());
    normalized
}

fn normalize_lookup(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn canonicalize_fact_end(value: &str, entity_alias_map: &HashMap<String, String>) -> String {
    let normalized = normalize_name(value);
    let lookup = normalize_lookup(&normalized);
    entity_alias_map.get(&lookup).cloned().unwrap_or(normalized)
}

fn normalize_predicate(predicate: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_separator = true;

    for ch in predicate.trim().chars() {
        if ch.is_ascii_alphanumeric() {
            if ch.is_ascii_uppercase() && !normalized.is_empty() && !previous_was_separator {
                normalized.push('_');
            }
            normalized.push(ch.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            normalized.push('_');
            previous_was_separator = true;
        }
    }

    normalized.trim_matches('_').to_string()
}

use anyhow::{Context, Result};
use lmkit::{
    create_chat_provider, ChatMessage, ChatProvider as LmkitChatProvider, ChatRequest,
    ProviderConfig, RequestPreset, ResponseFormat,
};
use memo_engine::{ExtractionProvider, ExtractionResult};
use serde::Deserialize;
use std::collections::HashMap;
use tokio::runtime::{Builder, Runtime};

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

const EXTRACTION_SYSTEM_PROMPT: &str = r#"You extract memory facts from user text.
Return strict JSON only.

Schema:
{
  "entities": [
    {
      "entity_type": "person|place|organization|project|pet|unknown",
      "name": "canonical entity name",
      "aliases": ["alias"],
      "confidence": 0.0
    }
  ],
  "facts": [
    {
      "subject": "entity name",
      "predicate": "snake_case_relation",
      "object": "entity or value text",
      "confidence": 0.0
    }
  ]
}

Rules:
- Output valid JSON object only, no prose.
- If nothing reliable is found, return {"entities":[],"facts":[]}.
- Keep confidence in [0,1].
- Use concise canonical names.
- Facts must be directly supported by the input text.
"#;

pub(crate) struct LmkitExtractionAdapter {
    runtime: Runtime,
    provider: Box<dyn LmkitChatProvider>,
    options: ExtractionCleanupOptions,
}

impl LmkitExtractionAdapter {
    pub(crate) fn new_with_options(
        config: ProviderConfig,
        options: ExtractionCleanupOptions,
    ) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime for lmkit extraction")?;
        let provider =
            create_chat_provider(&config).context("failed to create lmkit chat provider")?;

        Ok(Self {
            runtime,
            provider,
            options,
        })
    }
}

impl ExtractionProvider for LmkitExtractionAdapter {
    fn extract(&self, text: &str) -> Result<ExtractionResult> {
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system(EXTRACTION_SYSTEM_PROMPT),
                ChatMessage::user(text),
            ],
            response_format: Some(ResponseFormat::JsonObject),
            preset: Some(RequestPreset::Execution),
            temperature: Some(0.0),
            ..Default::default()
        };
        let response = self
            .runtime
            .block_on(self.provider.complete(&request))
            .context("lmkit extraction request failed")?;
        let content = response
            .content
            .as_deref()
            .with_context(|| "lmkit extraction response missing content".to_string())?;

        parse_extraction_response_with_options(content, self.options)
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

#[cfg(test)]
mod tests {
    use anyhow::Result;

    #[test]
    fn parse_extraction_response_reads_entities_and_facts() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": "Alice",
                        "aliases": ["Ally"],
                        "confidence": 0.91
                    }
                ],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "lives_in",
                        "object": "Paris",
                        "confidence": 0.88
                    }
                ]
            }"#,
            super::ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Alice");
        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].predicate, "lives_in");
        Ok(())
    }

    #[test]
    fn parse_extraction_response_accepts_json_code_fence() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            "```json\n{\"entities\":[],\"facts\":[]}\n```",
            super::ExtractionCleanupOptions::default(),
        )?;

        assert!(result.entities.is_empty());
        assert!(result.facts.is_empty());
        Ok(())
    }

    #[test]
    fn parse_extraction_response_normalizes_predicate_format() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            r#"{
                "entities": [],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "Lives In",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            super::ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.facts[0].predicate, "lives_in");
        Ok(())
    }

    #[test]
    fn parse_extraction_response_filters_low_confidence_and_empty_fields() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": "  ",
                        "aliases": ["A"],
                        "confidence": 0.9
                    },
                    {
                        "entity_type": "person",
                        "name": "Alice",
                        "aliases": [" Ally ", ""],
                        "confidence": 0.2
                    }
                ],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "lives_in",
                        "object": "Paris",
                        "confidence": 0.2
                    },
                    {
                        "subject": "Alice",
                        "predicate": "  ",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            super::ExtractionCleanupOptions::default(),
        )?;

        assert!(result.entities.is_empty());
        assert!(result.facts.is_empty());
        Ok(())
    }

    #[test]
    fn parse_extraction_response_extracts_json_object_from_wrapped_text() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            r#"Here is the extracted memory:

            {
              "entities": [
                {
                  "entity_type": "person",
                  "name": "Alice",
                  "aliases": ["Ally"],
                  "confidence": 0.92
                }
              ],
              "facts": [
                {
                  "subject": "Alice",
                  "predicate": "Lives In",
                  "object": "Paris",
                  "confidence": 0.9
                }
              ]
            }

            Hope this helps."#,
            super::ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Alice");
        assert_eq!(result.facts[0].predicate, "lives_in");
        Ok(())
    }

    #[test]
    fn parse_extraction_response_merges_duplicate_entities_and_facts() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": " Alice ",
                        "aliases": ["Ally"],
                        "confidence": 0.6
                    },
                    {
                        "entity_type": "person",
                        "name": "alice",
                        "aliases": ["A."],
                        "confidence": 0.9
                    }
                ],
                "facts": [
                    {
                        "subject": "Ally",
                        "predicate": "Lives In",
                        "object": " Paris ",
                        "confidence": 0.7
                    },
                    {
                        "subject": "alice",
                        "predicate": "lives_in",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            super::ExtractionCleanupOptions::default(),
        )?;

        assert_eq!(result.entities.len(), 1);
        assert_eq!(result.entities[0].name, "Alice");
        assert_eq!(result.entities[0].confidence, 0.9);
        assert!(result.entities[0].aliases.contains(&"Ally".to_string()));
        assert!(result.entities[0].aliases.contains(&"A.".to_string()));

        assert_eq!(result.facts.len(), 1);
        assert_eq!(result.facts[0].subject, "Alice");
        assert_eq!(result.facts[0].object, "Paris");
        assert_eq!(result.facts[0].predicate, "lives_in");
        assert_eq!(result.facts[0].confidence, 0.9);
        Ok(())
    }

    #[test]
    fn parse_extraction_response_respects_min_confidence_option() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            r#"{
                "entities": [
                    {
                        "entity_type": "person",
                        "name": "Alice",
                        "aliases": [],
                        "confidence": 0.65
                    }
                ],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "Lives In",
                        "object": "Paris",
                        "confidence": 0.65
                    }
                ]
            }"#,
            super::ExtractionCleanupOptions {
                min_confidence: 0.7,
                normalize_predicates: true,
            },
        )?;

        assert!(result.entities.is_empty());
        assert!(result.facts.is_empty());
        Ok(())
    }

    #[test]
    fn parse_extraction_response_can_disable_predicate_normalization() -> Result<()> {
        let result = super::parse_extraction_response_with_options(
            r#"{
                "entities": [],
                "facts": [
                    {
                        "subject": "Alice",
                        "predicate": "Lives In",
                        "object": "Paris",
                        "confidence": 0.9
                    }
                ]
            }"#,
            super::ExtractionCleanupOptions {
                min_confidence: 0.5,
                normalize_predicates: false,
            },
        )?;

        assert_eq!(result.facts[0].predicate, "Lives In");
        Ok(())
    }
}

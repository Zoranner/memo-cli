use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::{
    db::normalize_text,
    types::{EntityInput, EntityRecord, EpisodeInput, FactInput, RememberPreview},
    ExtractedEntity, ExtractedFact,
};

use super::MemoryEngine;

impl MemoryEngine {
    pub fn remember(&self, input: EpisodeInput) -> Result<String> {
        let preview = self.preview_remember(&input)?;

        let episode_vector = self.embed_if_available(&input.content)?;
        let episode = self.db.insert_episode(&input, episode_vector.as_deref())?;

        let mut entity_records = HashMap::<String, EntityRecord>::new();
        let mut mentioned_entity_ids = HashSet::<String>::new();
        for entity in preview.entities {
            let entity_vector = self.embed_if_available(&entity.name)?;
            let record = self.db.upsert_entity(
                &entity,
                input.layer,
                crate::db::ObservationContext {
                    source_episode_id: Some(&episode.id),
                    observed_at: episode.created_at,
                },
                entity_vector.as_deref(),
            )?;
            if mentioned_entity_ids.insert(record.id.clone()) {
                self.db
                    .add_mention(&episode.id, &record.id, "mentioned", entity.confidence)?;
            }
            entity_records.insert(normalize_text(&record.canonical_name), record);
        }

        for fact in preview.facts {
            let subject_key = normalize_text(&fact.subject);
            let object_key = normalize_text(&fact.object);
            let subject_record = if let Some(record) = entity_records.get(&subject_key) {
                record.clone()
            } else {
                let fallback = EntityInput {
                    entity_type: "unknown".to_string(),
                    name: fact.subject.clone(),
                    aliases: Vec::new(),
                    confidence: fact.confidence,
                    source: fact.source.clone(),
                };
                let vector = self.embed_if_available(&fallback.name)?;
                let record = self.db.upsert_entity(
                    &fallback,
                    input.layer,
                    crate::db::ObservationContext {
                        source_episode_id: Some(&episode.id),
                        observed_at: episode.created_at,
                    },
                    vector.as_deref(),
                )?;
                entity_records.insert(subject_key.clone(), record.clone());
                record
            };
            if mentioned_entity_ids.insert(subject_record.id.clone()) {
                self.db.add_mention(
                    &episode.id,
                    &subject_record.id,
                    "mentioned",
                    fact.confidence,
                )?;
            }
            let object_record = if let Some(record) = entity_records.get(&object_key) {
                record.clone()
            } else {
                let fallback = EntityInput {
                    entity_type: "unknown".to_string(),
                    name: fact.object.clone(),
                    aliases: Vec::new(),
                    confidence: fact.confidence,
                    source: fact.source.clone(),
                };
                let vector = self.embed_if_available(&fallback.name)?;
                let record = self.db.upsert_entity(
                    &fallback,
                    input.layer,
                    crate::db::ObservationContext {
                        source_episode_id: Some(&episode.id),
                        observed_at: episode.created_at,
                    },
                    vector.as_deref(),
                )?;
                entity_records.insert(object_key.clone(), record.clone());
                record
            };
            if mentioned_entity_ids.insert(object_record.id.clone()) {
                self.db.add_mention(
                    &episode.id,
                    &object_record.id,
                    "mentioned",
                    fact.confidence,
                )?;
            }

            let vector = self.embed_if_available(&format!(
                "{} {} {}",
                fact.subject, fact.predicate, fact.object
            ))?;
            self.db.insert_fact(
                &fact,
                input.layer,
                Some(&subject_record.id),
                Some(&object_record.id),
                crate::db::ObservationContext {
                    source_episode_id: Some(&episode.id),
                    observed_at: episode.created_at,
                },
                vector.as_deref(),
            )?;
            let _ = self.db.insert_edge(
                &subject_record.id,
                &fact.predicate,
                &object_record.id,
                fact.confidence,
                input.layer,
                crate::db::ObservationContext {
                    source_episode_id: Some(&episode.id),
                    observed_at: episode.created_at,
                },
            )?;
        }

        self.refresh_l3_cache()?;
        self.refresh_session_cache(&episode.id, &input.content, entity_records.values())?;

        Ok(episode.id)
    }

    pub fn preview_remember(&self, input: &EpisodeInput) -> Result<RememberPreview> {
        let extraction = self
            .config
            .extraction_provider
            .as_ref()
            .map(|provider| provider.extract(&input.content))
            .transpose()?
            .unwrap_or_default();

        Ok(RememberPreview {
            content: input.content.clone(),
            layer: input.layer,
            entities: merge_entities(input.entities.clone(), extraction.entities),
            facts: merge_facts(input.facts.clone(), extraction.facts),
            source_episode_id: input.source_episode_id.clone(),
            session_id: input.session_id.clone(),
            recorded_at: input.recorded_at,
            confidence: input.confidence,
        })
    }

    fn embed_if_available(&self, text: &str) -> Result<Option<Vec<f32>>> {
        let Some(provider) = &self.config.embedding_provider else {
            return Ok(None);
        };
        Ok(Some(provider.embed_text(text)?))
    }
}

fn merge_entities(manual: Vec<EntityInput>, extracted: Vec<ExtractedEntity>) -> Vec<EntityInput> {
    let mut merged: HashMap<String, EntityInput> = HashMap::new();
    for entity in manual {
        merged.insert(normalize_text(&entity.name), entity);
    }
    for entity in extracted {
        let key = normalize_text(&entity.name);
        merged
            .entry(key)
            .and_modify(|existing| {
                existing.confidence = existing.confidence.max(entity.confidence);
                for alias in &entity.aliases {
                    if !existing.aliases.iter().any(|item| item == alias) {
                        existing.aliases.push(alias.clone());
                    }
                }
            })
            .or_insert(EntityInput {
                entity_type: entity.entity_type,
                name: entity.name,
                aliases: entity.aliases,
                confidence: entity.confidence.max(0.5),
                source: crate::types::ExtractionSource::Provider,
            });
    }
    merged.into_values().collect()
}

fn merge_facts(manual: Vec<FactInput>, extracted: Vec<ExtractedFact>) -> Vec<FactInput> {
    let mut merged: HashMap<String, FactInput> = HashMap::new();
    for fact in manual {
        merged.insert(fact_key(&fact.subject, &fact.predicate, &fact.object), fact);
    }
    for fact in extracted {
        merged
            .entry(fact_key(&fact.subject, &fact.predicate, &fact.object))
            .or_insert(FactInput {
                subject: fact.subject,
                predicate: fact.predicate,
                object: fact.object,
                confidence: fact.confidence.max(0.5),
                source: crate::types::ExtractionSource::Provider,
            });
    }
    merged.into_values().collect()
}

fn fact_key(subject: &str, predicate: &str, object: &str) -> String {
    format!(
        "{}|{}|{}",
        normalize_text(subject),
        normalize_text(predicate),
        normalize_text(object)
    )
}

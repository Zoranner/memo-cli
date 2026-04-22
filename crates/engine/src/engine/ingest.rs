use std::collections::{HashMap, HashSet};

use anyhow::Result;

use crate::{
    db::normalize_text,
    types::{EntityInput, EntityRecord, EpisodeInput, FactInput, RememberPreview},
    ExtractedEntity, ExtractedFact, ExtractionResult,
};

use super::MemoryEngine;

#[derive(Default)]
pub(super) struct StructuredEpisodeSummary {
    pub entities: usize,
    pub facts: usize,
    pub mentions: usize,
    pub entity_records: HashMap<String, EntityRecord>,
}

impl StructuredEpisodeSummary {
    fn has_structure(&self) -> bool {
        self.entities > 0 || self.facts > 0 || self.mentions > 0
    }
}

impl MemoryEngine {
    pub fn remember(&self, input: EpisodeInput) -> Result<String> {
        let payload = self.build_remember_preview(&input, false)?;

        let episode_vector = self.embed_if_available(&input.content)?;
        let episode = self.db.insert_episode(&input, episode_vector.as_deref())?;
        let summary = self.ingest_episode_structure(&episode, payload.entities, payload.facts)?;
        if summary.has_structure() {
            self.db.mark_episode_structured(&episode.id)?;
        }

        self.refresh_l3_cache()?;
        self.refresh_session_cache(&episode.id, &input.content, summary.entity_records.values())?;

        Ok(episode.id)
    }

    fn resolve_fact_entity(
        &self,
        entity_records: &mut HashMap<String, EntityRecord>,
        episode: &crate::types::EpisodeRecord,
        fallback: EntityInput,
    ) -> Result<EntityRecord> {
        let key = normalize_text(&fallback.name);
        if let Some(record) = entity_records.get(&key) {
            return Ok(record.clone());
        }
        if let Some(record) = self.db.resolve_active_entity_reference(&fallback.name)? {
            cache_entity_record(entity_records, record.clone());
            return Ok(record);
        }

        let vector = self.embed_if_available(&fallback.name)?;
        let record = self.db.upsert_entity(
            &fallback,
            episode.layer,
            crate::db::ObservationContext {
                source_episode_id: Some(&episode.id),
                observed_at: episode.created_at,
            },
            vector.as_deref(),
        )?;
        cache_entity_record(entity_records, record.clone());
        Ok(record)
    }

    pub fn preview_remember(&self, input: &EpisodeInput) -> Result<RememberPreview> {
        self.build_remember_preview(input, true)
    }

    pub(super) fn structure_episode_with_provider(
        &self,
        episode: &crate::types::EpisodeRecord,
    ) -> Result<Option<StructuredEpisodeSummary>> {
        let Some(_) = &self.config.extraction_provider else {
            return Ok(None);
        };

        let extraction = self.extract_content(&episode.content)?;
        let summary = self.ingest_episode_structure(
            episode,
            merge_entities(Vec::new(), extraction.entities),
            merge_facts(Vec::new(), extraction.facts),
        )?;
        self.db.mark_episode_structured(&episode.id)?;
        Ok(Some(summary))
    }

    pub(super) fn ingest_episode_structure(
        &self,
        episode: &crate::types::EpisodeRecord,
        entities: Vec<EntityInput>,
        facts: Vec<FactInput>,
    ) -> Result<StructuredEpisodeSummary> {
        let mut summary = StructuredEpisodeSummary::default();
        let mut entity_records = HashMap::<String, EntityRecord>::new();
        let mut mentioned_entity_ids = HashSet::<String>::new();

        for entity in entities {
            let entity_vector = self.embed_if_available(&entity.name)?;
            let record = self.db.upsert_entity(
                &entity,
                episode.layer,
                crate::db::ObservationContext {
                    source_episode_id: Some(&episode.id),
                    observed_at: episode.created_at,
                },
                entity_vector.as_deref(),
            )?;
            if mentioned_entity_ids.insert(record.id.clone()) {
                self.db
                    .add_mention(&episode.id, &record.id, "mentioned", entity.confidence)?;
                summary.mentions += 1;
            }
            cache_entity_record(&mut entity_records, record);
            summary.entities += 1;
        }

        for fact in facts {
            let subject_record = self.resolve_fact_entity(
                &mut entity_records,
                episode,
                EntityInput {
                    entity_type: "unknown".to_string(),
                    name: fact.subject.clone(),
                    aliases: Vec::new(),
                    confidence: fact.confidence,
                    source: fact.source.clone(),
                },
            )?;
            if mentioned_entity_ids.insert(subject_record.id.clone()) {
                self.db.add_mention(
                    &episode.id,
                    &subject_record.id,
                    "mentioned",
                    fact.confidence,
                )?;
                summary.mentions += 1;
            }
            let object_record = self.resolve_fact_entity(
                &mut entity_records,
                episode,
                EntityInput {
                    entity_type: "unknown".to_string(),
                    name: fact.object.clone(),
                    aliases: Vec::new(),
                    confidence: fact.confidence,
                    source: fact.source.clone(),
                },
            )?;
            if mentioned_entity_ids.insert(object_record.id.clone()) {
                self.db.add_mention(
                    &episode.id,
                    &object_record.id,
                    "mentioned",
                    fact.confidence,
                )?;
                summary.mentions += 1;
            }

            let vector = self.embed_if_available(&format!(
                "{} {} {}",
                fact.subject, fact.predicate, fact.object
            ))?;
            self.db.insert_fact(
                &fact,
                episode.layer,
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
                episode.layer,
                crate::db::ObservationContext {
                    source_episode_id: Some(&episode.id),
                    observed_at: episode.created_at,
                },
            )?;
            summary.facts += 1;
        }

        summary.entity_records = entity_records;
        Ok(summary)
    }

    fn build_remember_preview(
        &self,
        input: &EpisodeInput,
        include_provider_extraction: bool,
    ) -> Result<RememberPreview> {
        let extraction = if include_provider_extraction {
            self.extract_content(&input.content)?
        } else {
            ExtractionResult::default()
        };

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

    fn extract_content(&self, text: &str) -> Result<ExtractionResult> {
        self.config
            .extraction_provider
            .as_ref()
            .map(|provider| provider.extract(text))
            .transpose()
            .map(|result| result.unwrap_or_default())
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

fn cache_entity_record(entity_records: &mut HashMap<String, EntityRecord>, record: EntityRecord) {
    entity_records.insert(normalize_text(&record.canonical_name), record.clone());
    for alias in &record.aliases {
        entity_records.insert(normalize_text(alias), record.clone());
    }
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use anyhow::Result;
    use tempfile::tempdir;

    use crate::{
        model::{ExtractionProvider, ExtractionResult},
        types::{EngineConfig, EpisodeInput, FactInput, MemoryLayer},
        ExtractedEntity, ExtractedFact, MemoryEngine,
    };

    #[derive(Clone)]
    struct StubExtractionProvider {
        result: ExtractionResult,
    }

    impl ExtractionProvider for StubExtractionProvider {
        fn extract(&self, _text: &str) -> Result<ExtractionResult> {
            Ok(self.result.clone())
        }
    }

    fn build_engine(temp_dir: &Path) -> Result<MemoryEngine> {
        let provider = StubExtractionProvider {
            result: ExtractionResult {
                entities: vec![
                    ExtractedEntity {
                        entity_type: "person".to_string(),
                        name: "Alice".to_string(),
                        aliases: vec!["Ally".to_string()],
                        confidence: 0.9,
                    },
                    ExtractedEntity {
                        entity_type: "organization".to_string(),
                        name: "Memo".to_string(),
                        aliases: Vec::new(),
                        confidence: 0.88,
                    },
                ],
                facts: vec![ExtractedFact {
                    subject: "Alice".to_string(),
                    predicate: "works_at".to_string(),
                    object: "Memo".to_string(),
                    confidence: 0.86,
                }],
            },
        };

        let config = EngineConfig::new(temp_dir).with_extraction_provider(Arc::new(provider));
        MemoryEngine::open(config)
    }

    #[test]
    fn remember_skips_provider_extraction_on_default_write_path() -> Result<()> {
        let temp_dir = tempdir()?;
        let engine = build_engine(temp_dir.path())?;

        let episode_id = engine.remember(EpisodeInput {
            content: "Alice works at Memo".to_string(),
            layer: MemoryLayer::L1,
            entities: Vec::new(),
            facts: Vec::new(),
            source_episode_id: None,
            session_id: None,
            recorded_at: None,
            confidence: 0.85,
        })?;

        let state = engine.state()?;
        assert_eq!(state.episode_count, 1);
        assert_eq!(state.entity_count, 0);
        assert_eq!(state.fact_count, 0);

        let record = engine.reflect(&episode_id)?.id().to_string();
        assert_eq!(record, episode_id);

        Ok(())
    }

    #[test]
    fn preview_remember_still_includes_provider_extraction() -> Result<()> {
        let temp_dir = tempdir()?;
        let engine = build_engine(temp_dir.path())?;

        let preview = engine.preview_remember(&EpisodeInput {
            content: "Alice works at Memo".to_string(),
            layer: MemoryLayer::L1,
            entities: Vec::new(),
            facts: vec![FactInput {
                subject: "Alice".to_string(),
                predicate: "works_at".to_string(),
                object: "Memo".to_string(),
                confidence: 0.92,
                source: crate::ExtractionSource::Manual,
            }],
            source_episode_id: None,
            session_id: None,
            recorded_at: None,
            confidence: 0.85,
        })?;

        assert_eq!(preview.entities.len(), 2);
        assert_eq!(preview.facts.len(), 1);
        assert!(preview.entities.iter().any(|entity| entity.name == "Alice"));
        assert!(preview.entities.iter().any(|entity| entity.name == "Memo"));

        Ok(())
    }
}

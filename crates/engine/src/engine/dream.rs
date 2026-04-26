use std::collections::{HashMap, HashSet};

use anyhow::Result;
use chrono::{Duration, Utc};
use tracing::warn;

use crate::types::{DreamReport, DreamTrigger, MemoryLayer, MemoryRecord};

use super::MemoryEngine;

impl MemoryEngine {
    pub fn dream(&self, trigger: DreamTrigger) -> Result<DreamReport> {
        self.run_dream(trigger)
    }

    pub fn dream_full(&self, trigger: DreamTrigger) -> Result<DreamReport> {
        const FULL_DREAM_MAX_PASSES: usize = 2;

        let mut merged = DreamReport::default();
        for pass in 0..FULL_DREAM_MAX_PASSES {
            let report = self.run_dream(trigger)?;
            let should_continue = dream_report_has_changes(&report);
            merge_dream_reports(&mut merged, report);
            if !should_continue || pass + 1 == FULL_DREAM_MAX_PASSES {
                break;
            }
        }
        Ok(merged)
    }

    fn run_dream(&self, trigger: DreamTrigger) -> Result<DreamReport> {
        const ENTITY_L3_MIN_SESSION_SPAN: Duration = Duration::days(1);
        const L3_STALE_AFTER: Duration = Duration::days(30);

        let mut report = DreamReport {
            trigger: trigger.as_str().to_string(),
            passes_run: 1,
            ..Default::default()
        };

        for group in self.db.duplicate_l1_episode_groups()? {
            if let Some(primary) = group.first() {
                report.promoted_to_l2 += self.promote_episode_cluster_to_l2(primary)?;
                for duplicate in group.iter().skip(1) {
                    report.archived_records += self.archive_episode_cluster(duplicate)?;
                    self.db.archive_record("episode", duplicate)?;
                    report.archived_records += 1;
                }
            }
        }

        self.structure_pending_episodes(&mut report)?;

        for episode_id in self.db.eligible_episode_ids_for_l2()? {
            report.promoted_to_l2 += self.promote_episode_cluster_to_l2(&episode_id)?;
        }

        for entity_id in self.db.eligible_entity_ids_for_l2_by_support()? {
            self.db
                .update_layer("entity", &entity_id, MemoryLayer::L2)?;
            report.promoted_to_l2 += 1;
        }

        report.promoted_to_l2 += self.promote_supported_fact_clusters_to_l2()?;
        let _ = self.backfill_mentions_from_active_facts()?;
        report.invalidated_records += self.invalidate_conflicting_facts()?;
        let (archived_duplicates, promoted_supported) = self.merge_supported_fact_clusters()?;
        report.archived_records += archived_duplicates;
        report.promoted_to_l3 += promoted_supported;

        for entity_id in self.db.active_entity_ids_in_layers(&[MemoryLayer::L2])? {
            let support_scopes = self.db.entity_support_scopes(&entity_id)?;
            if support_scopes.len() < 3 {
                continue;
            }
            let earliest = support_scopes
                .iter()
                .map(|(_, created_at)| *created_at)
                .min()
                .expect("entity support scopes should not be empty");
            let latest = support_scopes
                .iter()
                .map(|(_, created_at)| *created_at)
                .max()
                .expect("entity support scopes should not be empty");
            if latest - earliest < ENTITY_L3_MIN_SESSION_SPAN {
                continue;
            }

            self.db
                .update_layer("entity", &entity_id, MemoryLayer::L3)?;
            report.promoted_to_l3 += 1;
        }

        for kind in ["episode", "entity", "fact"] {
            for id in self.db.eligible_ids_for_l3(kind)? {
                self.db.update_layer(kind, &id, MemoryLayer::L3)?;
                report.promoted_to_l3 += 1;
            }
        }

        report.downgraded_records += self.cool_stale_l3_records(Utc::now() - L3_STALE_AFTER)?;

        self.refresh_l3_cache()?;
        Ok(report)
    }

    fn structure_pending_episodes(&self, report: &mut DreamReport) -> Result<()> {
        let Some(_) = &self.config.extraction_provider else {
            return Ok(());
        };

        for episode in self
            .db
            .load_unstructured_episodes(&[MemoryLayer::L1, MemoryLayer::L2])?
        {
            match self.structure_episode_with_provider(&episode) {
                Ok(Some(summary)) => {
                    report.structured_episodes += 1;
                    report.structured_entities += summary.entities;
                    report.structured_facts += summary.facts;
                }
                Ok(None) => return Ok(()),
                Err(error) => {
                    warn!(
                        episode_id = %episode.id,
                        error = %error,
                        "dream provider extraction failed; continuing with rule-based maintenance"
                    );
                    report.extraction_failures += 1;
                }
            }
        }

        Ok(())
    }

    fn backfill_mentions_from_active_facts(&self) -> Result<usize> {
        let facts =
            self.db
                .active_facts_in_layers(&[MemoryLayer::L1, MemoryLayer::L2, MemoryLayer::L3])?;
        let mut inserted = 0;

        for fact in facts {
            let Some(episode_id) = fact.source_episode_id.as_deref() else {
                continue;
            };

            for entity_id in [
                fact.subject_entity_id.as_deref(),
                fact.object_entity_id.as_deref(),
            ]
            .into_iter()
            .flatten()
            {
                if self
                    .db
                    .ensure_mention(episode_id, entity_id, "mentioned", fact.confidence)?
                {
                    inserted += 1;
                }
            }
        }

        Ok(inserted)
    }

    fn promote_episode_cluster_to_l2(&self, episode_id: &str) -> Result<usize> {
        let mut promoted = 0;
        self.db
            .update_layer("episode", episode_id, MemoryLayer::L2)?;
        promoted += 1;
        for kind in ["entity", "fact", "edge"] {
            for id in self.db.related_ids_for_episode(kind, episode_id)? {
                self.db.update_layer(kind, &id, MemoryLayer::L2)?;
                promoted += 1;
            }
        }
        Ok(promoted)
    }

    fn archive_episode_cluster(&self, episode_id: &str) -> Result<usize> {
        let mut archived = 0;
        for kind in ["fact", "edge"] {
            for id in self.db.related_ids_for_episode(kind, episode_id)? {
                self.db.archive_record(kind, &id)?;
                archived += 1;
            }
        }
        Ok(archived)
    }

    fn promote_supported_fact_clusters_to_l2(&self) -> Result<usize> {
        let fact_ids = self.db.eligible_fact_ids_for_l2_by_support()?;
        let mut promoted = 0;
        let mut promoted_edges = HashSet::new();

        for fact_id in fact_ids {
            let fact = match self.db.get_memory(&fact_id)? {
                Some(MemoryRecord::Fact(fact)) => fact,
                _ => continue,
            };
            self.db.update_layer("fact", &fact.id, MemoryLayer::L2)?;
            promoted += 1;

            if let (Some(subject_entity_id), Some(object_entity_id), Some(source_episode_id)) = (
                fact.subject_entity_id.as_deref(),
                fact.object_entity_id.as_deref(),
                fact.source_episode_id.as_deref(),
            ) {
                for edge_id in self.db.matching_edge_ids_for_source(
                    subject_entity_id,
                    &fact.predicate,
                    object_entity_id,
                    source_episode_id,
                    &[MemoryLayer::L1],
                )? {
                    if promoted_edges.insert(edge_id.clone()) {
                        self.db.update_layer("edge", &edge_id, MemoryLayer::L2)?;
                        promoted += 1;
                    }
                }
            }
        }

        Ok(promoted)
    }

    fn invalidate_conflicting_facts(&self) -> Result<usize> {
        let facts =
            self.db
                .active_facts_in_layers(&[MemoryLayer::L1, MemoryLayer::L2, MemoryLayer::L3])?;
        let mut groups = HashMap::<String, Vec<crate::types::FactRecord>>::new();
        for fact in facts {
            let key = format!(
                "{}|{}",
                crate::db::normalize_text(&fact.subject_text),
                crate::db::normalize_text(&fact.predicate)
            );
            groups.entry(key).or_default().push(fact);
        }

        let mut invalidated = 0;
        let mut conflict_source_episodes = HashSet::new();
        for facts in groups.into_values() {
            let unique_objects = facts
                .iter()
                .map(|fact| crate::db::normalize_text(&fact.object_text))
                .collect::<HashSet<_>>();
            if unique_objects.len() <= 1 {
                continue;
            }

            let winner = facts
                .iter()
                .max_by(|left, right| compare_fact_conflict_winner(left, right))
                .cloned()
                .expect("conflict group must contain at least one fact");
            let winner_object = crate::db::normalize_text(&winner.object_text);

            for fact in facts {
                if crate::db::normalize_text(&fact.object_text) == winner_object {
                    continue;
                }
                self.db.invalidate_record("fact", &fact.id)?;
                invalidated += 1;
                if let Some(source_episode_id) = fact.source_episode_id.as_deref() {
                    conflict_source_episodes.insert(source_episode_id.to_string());
                }

                if let (Some(subject_entity_id), Some(object_entity_id)) = (
                    fact.subject_entity_id.as_deref(),
                    fact.object_entity_id.as_deref(),
                ) {
                    for edge_id in self.db.matching_edge_ids(
                        subject_entity_id,
                        &fact.predicate,
                        object_entity_id,
                        &[MemoryLayer::L1, MemoryLayer::L2, MemoryLayer::L3],
                    )? {
                        self.db.invalidate_record("edge", &edge_id)?;
                        invalidated += 1;
                    }
                }
            }
        }
        for episode_id in conflict_source_episodes {
            if self.db.active_fact_count_for_episode(&episode_id)? == 0 {
                for kind in ["entity", "edge"] {
                    for id in self.db.active_related_ids_for_episode(kind, &episode_id)? {
                        self.db.invalidate_record(kind, &id)?;
                        invalidated += 1;
                    }
                }
                self.db.invalidate_record("episode", &episode_id)?;
                invalidated += 1;
            }
        }
        Ok(invalidated)
    }

    fn merge_supported_fact_clusters(&self) -> Result<(usize, usize)> {
        const FACT_L3_MIN_SESSION_SPAN: Duration = Duration::days(1);

        let facts = self
            .db
            .active_facts_in_layers(&[MemoryLayer::L2, MemoryLayer::L3])?;
        let mut groups = HashMap::<String, Vec<crate::types::FactRecord>>::new();
        for fact in facts {
            let key = format!(
                "{}|{}|{}",
                crate::db::normalize_text(&fact.subject_text),
                crate::db::normalize_text(&fact.predicate),
                crate::db::normalize_text(&fact.object_text)
            );
            groups.entry(key).or_default().push(fact);
        }

        let mut archived = 0;
        let mut promoted = 0;
        for facts in groups.into_values() {
            let mut distinct_sources = HashMap::new();
            for fact in &facts {
                let Some(episode_id) = fact.source_episode_id.as_deref() else {
                    continue;
                };
                if let Some((scope_key, created_at)) =
                    self.db.support_scope_for_episode(episode_id)?
                {
                    distinct_sources
                        .entry(scope_key)
                        .and_modify(|existing: &mut chrono::DateTime<Utc>| {
                            if created_at < *existing {
                                *existing = created_at;
                            }
                        })
                        .or_insert(created_at);
                }
            }
            if distinct_sources.len() < 3 {
                continue;
            }

            let earliest = distinct_sources
                .values()
                .min()
                .copied()
                .expect("fact support scopes should not be empty");
            let latest = distinct_sources
                .values()
                .max()
                .copied()
                .expect("fact support scopes should not be empty");
            if latest - earliest < FACT_L3_MIN_SESSION_SPAN {
                continue;
            }

            let winner = facts
                .iter()
                .max_by(|left, right| compare_fact_strength(left, right))
                .cloned()
                .expect("supported fact group must contain at least one fact");

            if winner.layer != MemoryLayer::L3 {
                self.db.update_layer("fact", &winner.id, MemoryLayer::L3)?;
                promoted += 1;
            }

            for fact in facts {
                if fact.id == winner.id {
                    continue;
                }
                self.db.archive_record("fact", &fact.id)?;
                archived += 1;

                if let (Some(subject_entity_id), Some(object_entity_id), Some(source_episode_id)) = (
                    fact.subject_entity_id.as_deref(),
                    fact.object_entity_id.as_deref(),
                    fact.source_episode_id.as_deref(),
                ) {
                    for edge_id in self.db.matching_edge_ids_for_source(
                        subject_entity_id,
                        &fact.predicate,
                        object_entity_id,
                        source_episode_id,
                        &[MemoryLayer::L2, MemoryLayer::L3],
                    )? {
                        self.db.archive_record("edge", &edge_id)?;
                        archived += 1;
                    }
                }
            }
        }

        Ok((archived, promoted))
    }

    fn cool_stale_l3_records(&self, stale_before: chrono::DateTime<Utc>) -> Result<usize> {
        let mut downgraded = 0;
        for record in self.db.load_all_l3_records()? {
            if !should_cool_l3_record(&record, stale_before) {
                continue;
            }
            self.db
                .update_layer(record.kind(), record.id(), MemoryLayer::L2)?;
            downgraded += 1;
        }
        Ok(downgraded)
    }

    pub(super) fn refresh_l3_cache(&self) -> Result<()> {
        let mut cache = self.l3_cache.lock().expect("l3 mutex poisoned");
        cache.clear();
        for record in self.db.load_l3_records(self.config.l3_cache_limit)? {
            cache.insert(record.id().to_string(), record);
        }
        Ok(())
    }
}

fn merge_dream_reports(target: &mut DreamReport, next: DreamReport) {
    if target.trigger.is_empty() {
        target.trigger = next.trigger.clone();
    }
    target.passes_run += next.passes_run;
    target.structured_episodes += next.structured_episodes;
    target.structured_entities += next.structured_entities;
    target.structured_facts += next.structured_facts;
    target.extraction_failures += next.extraction_failures;
    target.promoted_to_l2 += next.promoted_to_l2;
    target.promoted_to_l3 += next.promoted_to_l3;
    target.downgraded_records += next.downgraded_records;
    target.archived_records += next.archived_records;
    target.invalidated_records += next.invalidated_records;
}

fn dream_report_has_changes(report: &DreamReport) -> bool {
    report.structured_episodes > 0
        || report.structured_entities > 0
        || report.structured_facts > 0
        || report.promoted_to_l2 > 0
        || report.promoted_to_l3 > 0
        || report.downgraded_records > 0
        || report.archived_records > 0
        || report.invalidated_records > 0
}

fn should_cool_l3_record(record: &MemoryRecord, stale_before: chrono::DateTime<Utc>) -> bool {
    let Some(max_hit_count) = l3_cooldown_max_hit_count(record) else {
        return false;
    };
    record.hit_count() <= max_hit_count && record.activity_at() < stale_before
}

fn l3_cooldown_max_hit_count(record: &MemoryRecord) -> Option<u64> {
    match record {
        MemoryRecord::Episode(_) => Some(2),
        MemoryRecord::Entity(_) | MemoryRecord::Fact(_) => Some(1),
        MemoryRecord::Edge(_) => None,
    }
}

fn compare_fact_strength(
    left: &crate::types::FactRecord,
    right: &crate::types::FactRecord,
) -> std::cmp::Ordering {
    left.layer
        .boost()
        .total_cmp(&right.layer.boost())
        .then(left.hit_count.cmp(&right.hit_count))
        .then(left.confidence.total_cmp(&right.confidence))
        .then(left.updated_at.cmp(&right.updated_at))
        .then(left.created_at.cmp(&right.created_at))
}

fn compare_fact_conflict_winner(
    left: &crate::types::FactRecord,
    right: &crate::types::FactRecord,
) -> std::cmp::Ordering {
    left.confidence
        .total_cmp(&right.confidence)
        .then(left.updated_at.cmp(&right.updated_at))
        .then(left.created_at.cmp(&right.created_at))
        .then(left.hit_count.cmp(&right.hit_count))
        .then(left.layer.boost().total_cmp(&right.layer.boost()))
}

#[cfg(test)]
mod tests {
    use std::{path::Path, sync::Arc};

    use anyhow::{anyhow, Result};
    use chrono::{TimeZone, Utc};
    use tempfile::tempdir;

    use crate::model::{ExtractionProvider, ExtractionResult};
    use crate::types::{
        EdgeRecord, EngineConfig, EntityRecord, EpisodeInput, EpisodeRecord, FactRecord,
        MemoryLayer, MemoryRecord,
    };
    use crate::{DreamTrigger, ExtractedEntity, ExtractedFact, MemoryEngine};

    use super::{l3_cooldown_max_hit_count, should_cool_l3_record};

    #[derive(Clone)]
    struct StubExtractionProvider {
        result: Option<ExtractionResult>,
        error_message: Option<String>,
    }

    impl ExtractionProvider for StubExtractionProvider {
        fn extract(&self, _text: &str) -> Result<ExtractionResult> {
            if let Some(message) = &self.error_message {
                return Err(anyhow!(message.clone()));
            }
            Ok(self.result.clone().unwrap_or_default())
        }
    }

    fn build_engine(temp_dir: &Path, provider: StubExtractionProvider) -> Result<MemoryEngine> {
        let config = EngineConfig::new(temp_dir).with_extraction_provider(Arc::new(provider));
        MemoryEngine::open(config)
    }

    #[test]
    fn episode_l3_cooldown_allows_two_historical_hits() {
        let stale_before = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let activity_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let record = MemoryRecord::Episode(EpisodeRecord {
            id: "episode-1".to_string(),
            content: "Alice likes jasmine tea.".to_string(),
            layer: MemoryLayer::L3,
            confidence: 0.9,
            source_episode_id: None,
            session_id: None,
            created_at: activity_at,
            updated_at: activity_at,
            last_seen_at: activity_at,
            archived_at: None,
            invalidated_at: None,
            hit_count: 2,
        });

        assert_eq!(l3_cooldown_max_hit_count(&record), Some(2));
        assert!(should_cool_l3_record(&record, stale_before));
    }

    #[test]
    fn entity_l3_cooldown_remains_stricter_than_episode() {
        let stale_before = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let activity_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let record = MemoryRecord::Entity(EntityRecord {
            id: "entity-1".to_string(),
            entity_type: "person".to_string(),
            canonical_name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
            layer: MemoryLayer::L3,
            confidence: 0.95,
            source_episode_id: None,
            created_at: activity_at,
            updated_at: activity_at,
            last_seen_at: activity_at,
            archived_at: None,
            invalidated_at: None,
            hit_count: 2,
        });

        assert_eq!(l3_cooldown_max_hit_count(&record), Some(1));
        assert!(!should_cool_l3_record(&record, stale_before));
    }

    #[test]
    fn fact_l3_cooldown_still_requires_low_hits() {
        let stale_before = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let activity_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let record = MemoryRecord::Fact(FactRecord {
            id: "fact-1".to_string(),
            subject_entity_id: Some("alice".to_string()),
            subject_text: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object_entity_id: Some("paris".to_string()),
            object_text: "Paris".to_string(),
            layer: MemoryLayer::L3,
            confidence: 0.95,
            source_episode_id: None,
            created_at: activity_at,
            updated_at: activity_at,
            valid_from: Some(activity_at),
            valid_to: None,
            archived_at: None,
            invalidated_at: None,
            hit_count: 2,
        });

        assert_eq!(l3_cooldown_max_hit_count(&record), Some(1));
        assert!(!should_cool_l3_record(&record, stale_before));
    }

    #[test]
    fn edge_is_excluded_from_l3_cooldown() {
        let activity_at = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        let stale_before = Utc.with_ymd_and_hms(2026, 2, 1, 0, 0, 0).unwrap();
        let record = MemoryRecord::Edge(EdgeRecord {
            id: "edge-1".to_string(),
            subject_entity_id: "alice".to_string(),
            predicate: "lives_in".to_string(),
            object_entity_id: "paris".to_string(),
            weight: 0.9,
            source_episode_id: None,
            layer: MemoryLayer::L3,
            valid_from: Some(activity_at),
            valid_to: None,
            created_at: activity_at,
            updated_at: activity_at,
            archived_at: None,
            invalidated_at: None,
            hit_count: 0,
        });

        assert_eq!(l3_cooldown_max_hit_count(&record), None);
        assert!(!should_cool_l3_record(&record, stale_before));
    }

    #[test]
    fn dream_structures_unstructured_episodes_via_provider_extraction() -> Result<()> {
        let temp_dir = tempdir()?;
        let engine = build_engine(
            temp_dir.path(),
            StubExtractionProvider {
                result: Some(ExtractionResult {
                    entities: vec![
                        ExtractedEntity {
                            entity_type: "person".to_string(),
                            name: "Alice".to_string(),
                            aliases: vec!["Ally".to_string()],
                            confidence: 0.91,
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
                }),
                error_message: None,
            },
        )?;

        engine.remember(EpisodeInput {
            content: "Alice works at Memo".to_string(),
            layer: MemoryLayer::L1,
            entities: Vec::new(),
            facts: Vec::new(),
            source_episode_id: None,
            session_id: None,
            recorded_at: None,
            confidence: 0.85,
        })?;

        let first_report = engine.dream(DreamTrigger::Manual)?;
        let first_state = engine.state()?;
        assert_eq!(first_report.structured_episodes, 1);
        assert_eq!(first_report.structured_entities, 2);
        assert_eq!(first_report.structured_facts, 1);
        assert_eq!(first_state.entity_count, 2);
        assert_eq!(first_state.fact_count, 1);

        let second_report = engine.dream(DreamTrigger::Manual)?;
        let second_state = engine.state()?;
        assert_eq!(second_report.structured_episodes, 0);
        assert_eq!(second_report.structured_entities, 0);
        assert_eq!(second_report.structured_facts, 0);
        assert_eq!(second_state.entity_count, 2);
        assert_eq!(second_state.fact_count, 1);

        Ok(())
    }

    #[test]
    fn dream_degrades_gracefully_when_provider_extraction_fails() -> Result<()> {
        let temp_dir = tempdir()?;
        let engine = build_engine(
            temp_dir.path(),
            StubExtractionProvider {
                result: None,
                error_message: Some("provider offline".to_string()),
            },
        )?;

        engine.remember(EpisodeInput {
            content: "Alice works at Memo".to_string(),
            layer: MemoryLayer::L1,
            entities: Vec::new(),
            facts: Vec::new(),
            source_episode_id: None,
            session_id: None,
            recorded_at: None,
            confidence: 0.85,
        })?;

        let report = engine.dream(DreamTrigger::Manual)?;
        let state = engine.state()?;
        assert_eq!(report.extraction_failures, 1);
        assert_eq!(report.structured_episodes, 0);
        assert_eq!(state.entity_count, 0);
        assert_eq!(state.fact_count, 0);

        Ok(())
    }
}

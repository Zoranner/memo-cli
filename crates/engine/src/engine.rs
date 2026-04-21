use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::Instant,
};

use anyhow::{Context, Result};
use chrono::{Duration, Utc};
use tracing::debug;

use crate::{
    db::{normalize_text, Database, ObservationContext},
    text_index::TextIndex,
    types::{
        DreamReport, DreamTrigger, EngineConfig, EntityInput, EntityRecord, EpisodeInput,
        FactInput, MemoryLayer, MemoryRecord, RecallReason, RecallRequest, RecallResult,
        RecallResultSet, RememberPreview, RestoreReport, RestoreScope, SystemState,
    },
    vector_index::VectorIndex,
    ExtractedEntity, ExtractedFact,
};

pub type Engine = MemoryEngine;

pub struct MemoryEngine {
    config: EngineConfig,
    db: Database,
    text_index: Mutex<TextIndex>,
    vector_index: Mutex<VectorIndex>,
    l3_cache: Mutex<HashMap<String, MemoryRecord>>,
    session: Mutex<SessionCache>,
}

#[derive(Default)]
struct SessionCache {
    recent_aliases: HashMap<String, String>,
    recent_memory_ids: Vec<String>,
    recent_topics: Vec<String>,
}

#[derive(Clone)]
struct Candidate {
    memory: MemoryRecord,
    score: f32,
    reasons: Vec<RecallReason>,
}

impl MemoryEngine {
    pub fn open(config: EngineConfig) -> Result<Self> {
        config.ensure_dirs()?;

        let db = Database::open(&config.sqlite_path())?;
        let text_index = TextIndex::open(&config.text_index_dir())?;
        let vector_index = VectorIndex::open(config.vector_index_path(), config.vector_dimension)?;

        let engine = Self {
            config,
            db,
            text_index: Mutex::new(text_index),
            vector_index: Mutex::new(vector_index),
            l3_cache: Mutex::new(HashMap::new()),
            session: Mutex::new(SessionCache::default()),
        };

        engine.restore_full(RestoreScope::All)?;
        engine.refresh_l3_cache()?;
        Ok(engine)
    }

    pub fn remember(&self, input: EpisodeInput) -> Result<String> {
        let preview = self.preview_remember(&input)?;

        let episode_vector = self.embed_if_available(&input.content)?;
        let episode = self.db.insert_episode(&input, episode_vector.as_deref())?;

        let mut entity_records = HashMap::<String, EntityRecord>::new();
        for entity in preview.entities {
            let entity_vector = self.embed_if_available(&entity.name)?;
            let record = self.db.upsert_entity(
                &entity,
                input.layer,
                ObservationContext {
                    source_episode_id: Some(&episode.id),
                    observed_at: episode.created_at,
                },
                entity_vector.as_deref(),
            )?;
            self.db
                .add_mention(&episode.id, &record.id, "mentioned", entity.confidence)?;
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
                    ObservationContext {
                        source_episode_id: Some(&episode.id),
                        observed_at: episode.created_at,
                    },
                    vector.as_deref(),
                )?;
                entity_records.insert(subject_key.clone(), record.clone());
                record
            };
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
                    ObservationContext {
                        source_episode_id: Some(&episode.id),
                        observed_at: episode.created_at,
                    },
                    vector.as_deref(),
                )?;
                entity_records.insert(object_key.clone(), record.clone());
                record
            };

            let vector = self.embed_if_available(&format!(
                "{} {} {}",
                fact.subject, fact.predicate, fact.object
            ))?;
            self.db.insert_fact(
                &fact,
                input.layer,
                Some(&subject_record.id),
                Some(&object_record.id),
                ObservationContext {
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
                ObservationContext {
                    source_episode_id: Some(&episode.id),
                    observed_at: episode.created_at,
                },
            )?;
        }

        self.mark_indexes_pending()?;
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

    pub fn recall(&self, request: RecallRequest) -> Result<RecallResultSet> {
        let started = Instant::now();
        let normalized = normalize_text(&request.query);
        let mut result = self.execute_query(&request, request.deep)?;
        if !request.deep && should_auto_escalate_to_deep_search(&result) {
            result = self.execute_query(&request, true)?;
        }

        self.commit_query_results(&normalized, &result.results)?;

        debug!(
            query = %request.query,
            deep = result.deep_search_used,
            candidates = result.results.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "memory query completed"
        );

        Ok(result)
    }

    fn execute_query(&self, request: &RecallRequest, deep: bool) -> Result<RecallResultSet> {
        let mut candidates: HashMap<String, Candidate> = HashMap::new();
        let limit = request.limit.max(1);
        let text_limit = if deep { limit * 12 } else { limit * 6 };
        let graph_limit = if deep { limit * 8 } else { limit * 4 };
        let graph_hops = if deep { 2 } else { 1 };
        let normalized = normalize_text(&request.query);

        if let Some(candidate) = self.l0_match(&normalized)? {
            add_candidate(&mut candidates, candidate);
        }

        for candidate in self.l3_matches(&normalized)? {
            add_candidate(&mut candidates, candidate);
        }

        for record in self.db.search_exact_alias(&request.query)? {
            let reason = match &record {
                MemoryRecord::Entity(_) => RecallReason::Alias,
                _ => RecallReason::Exact,
            };
            add_candidate(
                &mut candidates,
                Candidate {
                    memory: record,
                    score: 3.0,
                    reasons: vec![reason],
                },
            );
        }

        {
            let text_index = self.text_index.lock().expect("tantivy mutex poisoned");
            for hit in text_index.search(&request.query, text_limit)? {
                if let Some(memory) = self.db.get_memory(&hit.id)? {
                    add_candidate(
                        &mut candidates,
                        Candidate {
                            memory,
                            score: 0.4 + hit.score.max(0.0) * 0.15,
                            reasons: vec![RecallReason::Bm25],
                        },
                    );
                }
            }
        }

        if let Some(provider) = &self.config.embedding_provider {
            let query_vector = provider.embed_text(&request.query)?;
            let vector_index = self.vector_index.lock().expect("vector mutex poisoned");
            for hit in vector_index.search(&query_vector, text_limit)? {
                if let Some(memory) = self.db.get_memory(&hit.id)? {
                    add_candidate(
                        &mut candidates,
                        Candidate {
                            memory,
                            score: hit.score.max(0.0) * 1.2,
                            reasons: vec![RecallReason::Vector],
                        },
                    );
                }
            }
        }

        let graph_seeds =
            collect_graph_seeds(candidates.values().map(|candidate| &candidate.memory));
        for (memory, hops) in
            self.db
                .related_graph_records(&graph_seeds, graph_hops, graph_limit)?
        {
            add_candidate(
                &mut candidates,
                Candidate {
                    memory,
                    score: 0.35 / hops as f32,
                    reasons: vec![RecallReason::GraphHop { hops }],
                },
            );
        }

        let mut scored: Vec<Candidate> = candidates
            .into_values()
            .map(|mut candidate| {
                let recency = recency_boost(candidate.memory.activity_at());
                if recency > 0.0 {
                    candidate.score += recency;
                    candidate.reasons.push(RecallReason::RecencyBoost);
                }
                let layer_boost = candidate.memory.layer().boost();
                if layer_boost > 0.0 {
                    candidate.score += layer_boost;
                    candidate.reasons.push(RecallReason::LayerBoost);
                }
                let frequency_boost = hit_frequency_boost(candidate.memory.hit_count());
                if frequency_boost > 0.0 {
                    candidate.score += frequency_boost;
                    candidate.reasons.push(RecallReason::HitFrequencyBoost);
                }
                candidate
            })
            .collect();
        self.apply_rerank(deep, &request.query, limit, &mut scored)?;
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));

        let selected = mmr_select(scored, limit);
        let results = selected
            .into_iter()
            .map(|mut candidate| {
                candidate.reasons.push(RecallReason::MmrSelected);
                RecallResult {
                    memory: candidate.memory,
                    score: candidate.score,
                    reasons: candidate.reasons,
                }
            })
            .collect::<Vec<_>>();

        Ok(RecallResultSet {
            total_candidates: results.len(),
            deep_search_used: deep,
            results,
        })
    }

    pub fn reflect(&self, id: &str) -> Result<MemoryRecord> {
        self.db
            .get_memory(id)?
            .with_context(|| format!("memory not found: {}", id))
    }

    pub fn dream(&self, trigger: DreamTrigger) -> Result<DreamReport> {
        let job_id = self.db.create_dream_job(trigger.as_str(), "running")?;
        let report = self.run_dream(trigger);
        match report {
            Ok(report) => {
                self.db.complete_dream_job(&job_id)?;
                Ok(report)
            }
            Err(error) => {
                let _ = self.db.fail_dream_job(&job_id);
                Err(error)
            }
        }
    }

    pub fn dream_full(&self, trigger: DreamTrigger) -> Result<DreamReport> {
        let mut report = self.dream(trigger)?;
        let pending = self.db.dream_job_stats()?.pending;
        if pending == 0 {
            return Ok(report);
        }

        for queued_report in self.run_pending_dreams(pending)? {
            merge_dream_reports(&mut report, queued_report);
        }

        Ok(report)
    }

    pub fn schedule_dream(&self, trigger: DreamTrigger) -> Result<String> {
        self.db.create_dream_job(trigger.as_str(), "pending")
    }

    pub fn run_pending_dreams(&self, limit: usize) -> Result<Vec<DreamReport>> {
        let mut reports = Vec::new();
        for (job_id, trigger) in self.db.claim_pending_dream_jobs(limit.max(1))? {
            let trigger = trigger.parse::<DreamTrigger>()?;
            match self.run_dream(trigger) {
                Ok(report) => {
                    self.db.complete_dream_job(&job_id)?;
                    reports.push(report);
                }
                Err(error) => {
                    let _ = self.db.fail_dream_job(&job_id);
                    return Err(error);
                }
            }
        }
        Ok(reports)
    }

    fn run_dream(&self, trigger: DreamTrigger) -> Result<DreamReport> {
        const ENTITY_L3_MIN_SESSION_SPAN: Duration = Duration::days(1);
        const L3_STALE_AFTER: Duration = Duration::days(30);

        let mut report = DreamReport {
            trigger: trigger.as_str().to_string(),
            jobs_processed: 1,
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

        for episode_id in self.db.eligible_episode_ids_for_l2()? {
            report.promoted_to_l2 += self.promote_episode_cluster_to_l2(&episode_id)?;
        }

        for entity_id in self.db.eligible_entity_ids_for_l2_by_support()? {
            self.db
                .update_layer("entity", &entity_id, MemoryLayer::L2)?;
            report.promoted_to_l2 += 1;
        }

        report.promoted_to_l2 += self.promote_supported_fact_clusters_to_l2()?;
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

    fn apply_rerank(
        &self,
        deep: bool,
        query: &str,
        limit: usize,
        candidates: &mut [Candidate],
    ) -> Result<()> {
        if !deep || candidates.len() < 2 {
            return Ok(());
        }
        let Some(provider) = &self.config.rerank_provider else {
            return Ok(());
        };

        candidates.sort_by(|a, b| b.score.total_cmp(&a.score));
        let rerank_limit = candidates.len().min(limit.max(1) * 4);
        let documents = candidates
            .iter()
            .take(rerank_limit)
            .map(|candidate| candidate.memory.text_for_ranking())
            .collect::<Vec<_>>();
        let reranked = provider.rerank(query, &documents)?;

        for item in reranked {
            if item.index >= rerank_limit {
                continue;
            }
            let candidate = &mut candidates[item.index];
            candidate.score += 5.0 + item.score.max(0.0);
            candidate.reasons.push(RecallReason::Rerank);
        }

        Ok(())
    }

    fn commit_query_results(&self, normalized_query: &str, results: &[RecallResult]) -> Result<()> {
        for result in results {
            let _ = self.db.increment_hit_count(&result.memory);
        }
        self.record_query_session(normalized_query, results)
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
        let facts = self
            .db
            .active_facts_in_layers(&[MemoryLayer::L2, MemoryLayer::L3])?;
        let mut groups = HashMap::<String, Vec<crate::types::FactRecord>>::new();
        for fact in facts {
            let key = format!(
                "{}|{}",
                normalize_text(&fact.subject_text),
                normalize_text(&fact.predicate)
            );
            groups.entry(key).or_default().push(fact);
        }

        let mut invalidated = 0;
        for facts in groups.into_values() {
            let unique_objects = facts
                .iter()
                .map(|fact| normalize_text(&fact.object_text))
                .collect::<HashSet<_>>();
            if unique_objects.len() <= 1 {
                continue;
            }

            let winner_id = facts
                .iter()
                .max_by(|left, right| compare_fact_strength(left, right))
                .map(|fact| fact.id.clone())
                .expect("conflict group must contain at least one fact");

            for fact in facts {
                if fact.id == winner_id {
                    continue;
                }
                self.db.invalidate_record("fact", &fact.id)?;
                invalidated += 1;

                if let (Some(subject_entity_id), Some(object_entity_id)) = (
                    fact.subject_entity_id.as_deref(),
                    fact.object_entity_id.as_deref(),
                ) {
                    for edge_id in self.db.matching_edge_ids(
                        subject_entity_id,
                        &fact.predicate,
                        object_entity_id,
                        &[MemoryLayer::L2, MemoryLayer::L3],
                    )? {
                        self.db.invalidate_record("edge", &edge_id)?;
                        invalidated += 1;
                    }
                }
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
                normalize_text(&fact.subject_text),
                normalize_text(&fact.predicate),
                normalize_text(&fact.object_text)
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

    pub fn restore_full(&self, scope: RestoreScope) -> Result<RestoreReport> {
        let mut report = RestoreReport::default();

        if matches!(scope, RestoreScope::All | RestoreScope::Text) {
            let docs = self.db.load_search_documents()?;
            let count = self
                .text_index
                .lock()
                .expect("tantivy mutex poisoned")
                .rebuild(&docs)?;
            self.db
                .record_index_state("text", count, "ready", Some("tantivy rebuild complete"))?;
            report.text_documents = count;
        }

        if matches!(scope, RestoreScope::All | RestoreScope::Vector) {
            let docs = self.db.load_vector_documents()?;
            let count = self
                .vector_index
                .lock()
                .expect("vector mutex poisoned")
                .rebuild(&docs)?;
            self.db.record_index_state(
                "vector",
                count,
                "ready",
                Some("vector rebuild complete"),
            )?;
            report.vector_documents = count;
        }

        self.refresh_l3_cache()?;
        Ok(report)
    }

    pub fn restore(&self, scope: RestoreScope) -> Result<RestoreReport> {
        match scope {
            RestoreScope::All => self.restore_full(RestoreScope::All),
            RestoreScope::Text => {
                if self.db.index_status("text")?.status == "pending" {
                    self.restore_full(RestoreScope::Text)
                } else {
                    Ok(RestoreReport::default())
                }
            }
            RestoreScope::Vector => {
                if self.db.index_status("vector")?.status == "pending" {
                    self.restore_full(RestoreScope::Vector)
                } else {
                    Ok(RestoreReport::default())
                }
            }
        }
    }

    pub fn state(&self) -> Result<SystemState> {
        let (episode_count, entity_count, fact_count, edge_count) = self.db.stats()?;
        Ok(SystemState {
            episode_count,
            entity_count,
            fact_count,
            edge_count,
            l3_cached: self.l3_cache.lock().expect("l3 mutex poisoned").len(),
            dream_jobs: self.db.dream_job_stats()?,
            text_index: self.db.index_status("text")?,
            vector_index: self.db.index_status("vector")?,
        })
    }

    fn refresh_l3_cache(&self) -> Result<()> {
        let mut cache = self.l3_cache.lock().expect("l3 mutex poisoned");
        cache.clear();
        for record in self.db.load_l3_records(self.config.l3_cache_limit)? {
            cache.insert(record.id().to_string(), record);
        }
        Ok(())
    }

    fn l0_match(&self, normalized_query: &str) -> Result<Option<Candidate>> {
        let session = self.session.lock().expect("session mutex poisoned");
        let Some(entity_id) = session.recent_aliases.get(normalized_query).cloned() else {
            return Ok(None);
        };
        drop(session);
        let memory = self
            .db
            .get_memory(&entity_id)?
            .with_context(|| format!("dangling L0 entity reference: {}", entity_id))?;
        Ok(Some(Candidate {
            memory,
            score: 3.5,
            reasons: vec![RecallReason::L0],
        }))
    }

    fn l3_matches(&self, normalized_query: &str) -> Result<Vec<Candidate>> {
        let cache = self.l3_cache.lock().expect("l3 mutex poisoned");
        let mut result = Vec::new();
        for record in cache.values() {
            let haystack = normalize_text(&record.text_for_ranking());
            if haystack.contains(normalized_query) {
                result.push(Candidate {
                    memory: record.clone(),
                    score: 2.4,
                    reasons: vec![RecallReason::L3],
                });
            }
        }
        Ok(result)
    }

    fn refresh_session_cache<'a>(
        &self,
        episode_id: &str,
        content: &str,
        entities: impl Iterator<Item = &'a EntityRecord>,
    ) -> Result<()> {
        let mut session = self.session.lock().expect("session mutex poisoned");
        session.recent_memory_ids.push(episode_id.to_string());
        session.recent_topics.push(normalize_text(content));
        for entity in entities {
            session
                .recent_aliases
                .insert(normalize_text(&entity.canonical_name), entity.id.clone());
            for alias in &entity.aliases {
                session
                    .recent_aliases
                    .insert(normalize_text(alias), entity.id.clone());
            }
        }
        if session.recent_memory_ids.len() > 128 {
            session.recent_memory_ids.drain(..64);
        }
        if session.recent_topics.len() > 64 {
            session.recent_topics.drain(..32);
        }
        Ok(())
    }

    fn record_query_session(&self, normalized_query: &str, results: &[RecallResult]) -> Result<()> {
        let mut session = self.session.lock().expect("session mutex poisoned");
        session.recent_topics.push(normalized_query.to_string());
        for result in results {
            session
                .recent_memory_ids
                .push(result.memory.id().to_string());
            if let MemoryRecord::Entity(entity) = &result.memory {
                session
                    .recent_aliases
                    .insert(normalize_text(&entity.canonical_name), entity.id.clone());
                for alias in &entity.aliases {
                    session
                        .recent_aliases
                        .insert(normalize_text(alias), entity.id.clone());
                }
            }
        }
        if session.recent_memory_ids.len() > 128 {
            session.recent_memory_ids.drain(..64);
        }
        Ok(())
    }

    fn embed_if_available(&self, text: &str) -> Result<Option<Vec<f32>>> {
        let Some(provider) = &self.config.embedding_provider else {
            return Ok(None);
        };
        Ok(Some(provider.embed_text(text)?))
    }

    fn mark_indexes_pending(&self) -> Result<()> {
        self.db.record_index_state(
            "text",
            self.db.load_search_documents()?.len(),
            "pending",
            Some("pending restore after remember"),
        )?;
        if self.config.embedding_provider.is_some() {
            self.db.record_index_state(
                "vector",
                self.db.load_vector_documents()?.len(),
                "pending",
                Some("pending restore after remember"),
            )?;
        }
        Ok(())
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

fn collect_graph_seeds<'a>(records: impl Iterator<Item = &'a MemoryRecord>) -> Vec<String> {
    let mut ids = HashSet::new();
    for record in records {
        match record {
            MemoryRecord::Entity(entity) => {
                ids.insert(entity.id.clone());
            }
            MemoryRecord::Fact(fact) => {
                if let Some(id) = &fact.subject_entity_id {
                    ids.insert(id.clone());
                }
                if let Some(id) = &fact.object_entity_id {
                    ids.insert(id.clone());
                }
            }
            _ => {}
        }
    }
    ids.into_iter().collect()
}

fn add_candidate(target: &mut HashMap<String, Candidate>, candidate: Candidate) {
    let key = format!("{}:{}", candidate.memory.kind(), candidate.memory.id());
    target
        .entry(key)
        .and_modify(|existing| {
            existing.score = existing.score.max(candidate.score);
            existing.reasons.extend(candidate.reasons.iter().cloned());
        })
        .or_insert(candidate);
}

fn merge_dream_reports(target: &mut DreamReport, next: DreamReport) {
    target.promoted_to_l2 += next.promoted_to_l2;
    target.promoted_to_l3 += next.promoted_to_l3;
    target.downgraded_records += next.downgraded_records;
    target.archived_records += next.archived_records;
    target.invalidated_records += next.invalidated_records;
    target.jobs_processed += next.jobs_processed;
}

fn recency_boost(updated_at: chrono::DateTime<Utc>) -> f32 {
    let age_days = (Utc::now() - updated_at).num_days().max(0) as f32;
    (-(age_days / 30.0)).exp() * 0.18
}

fn hit_frequency_boost(hit_count: u64) -> f32 {
    ((hit_count as f32) + 1.0).ln() * 0.05
}

fn should_auto_escalate_to_deep_search(result: &RecallResultSet) -> bool {
    if result.results.len() < 2 {
        return false;
    }

    let first = &result.results[0];
    if first.reasons.iter().any(|reason| {
        matches!(
            reason,
            RecallReason::L0 | RecallReason::L3 | RecallReason::Exact | RecallReason::Alias
        )
    }) {
        return false;
    }

    let second = &result.results[1];
    let score_gap = (first.score - second.score).abs();
    score_gap <= 0.25
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

fn mmr_select(mut candidates: Vec<Candidate>, limit: usize) -> Vec<Candidate> {
    if candidates.len() <= limit {
        return candidates;
    }

    let mut selected = Vec::new();
    while !candidates.is_empty() && selected.len() < limit {
        let (best_index, _) = candidates
            .iter()
            .enumerate()
            .map(|(index, candidate)| {
                let novelty_penalty = selected
                    .iter()
                    .map(|existing: &Candidate| {
                        text_similarity(
                            existing.memory.text_for_ranking(),
                            candidate.memory.text_for_ranking(),
                        )
                    })
                    .fold(0.0_f32, f32::max);
                let score = 0.7 * candidate.score - 0.3 * novelty_penalty;
                (index, score)
            })
            .max_by(|a, b| a.1.total_cmp(&b.1))
            .expect("candidate set is non-empty");
        selected.push(candidates.remove(best_index));
    }
    selected
}

fn text_similarity(a: String, b: String) -> f32 {
    let a_tokens: HashSet<_> = normalize_text(&a)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    let b_tokens: HashSet<_> = normalize_text(&b)
        .split_whitespace()
        .map(str::to_string)
        .collect();
    if a_tokens.is_empty() || b_tokens.is_empty() {
        return 0.0;
    }
    let intersection = a_tokens.intersection(&b_tokens).count() as f32;
    let union = a_tokens.union(&b_tokens).count() as f32;
    if union == 0.0 {
        0.0
    } else {
        intersection / union
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};

    use crate::types::{
        EdgeRecord, EntityRecord, EpisodeRecord, FactRecord, MemoryLayer, MemoryRecord,
    };

    use super::{l3_cooldown_max_hit_count, should_cool_l3_record};

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
}

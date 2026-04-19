use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
    time::Instant,
};

use anyhow::{Context, Result};
use chrono::Utc;
use tracing::debug;

use crate::{
    db::{normalize_text, Database},
    text_index::TextIndex,
    types::{
        ConsolidationReport, ConsolidationTrigger, EngineConfig, EngineStats, EntityInput,
        EntityRecord, EpisodeInput, FactInput, MemoryLayer, MemoryRecord, QueryResultSet,
        RebuildReport, RebuildScope, RetrieveReason, RetrieveRequest, RetrieveResult,
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
    reasons: Vec<RetrieveReason>,
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

        engine.rebuild_indexes(RebuildScope::All)?;
        engine.refresh_l3_cache()?;
        Ok(engine)
    }

    pub fn ingest_episode(&self, input: EpisodeInput) -> Result<String> {
        let extraction = self
            .config
            .extraction_provider
            .as_ref()
            .map(|provider| provider.extract(&input.content))
            .transpose()?
            .unwrap_or_default();

        let entities = merge_entities(input.entities.clone(), extraction.entities);
        let facts = merge_facts(input.facts.clone(), extraction.facts);

        let episode_vector = self.embed_if_available(&input.content)?;
        let episode = self.db.insert_episode(&input, episode_vector.as_deref())?;

        let mut entity_records = HashMap::<String, EntityRecord>::new();
        for entity in entities {
            let entity_vector = self.embed_if_available(&entity.name)?;
            let record = self.db.upsert_entity(
                &entity,
                input.layer,
                Some(&episode.id),
                entity_vector.as_deref(),
            )?;
            self.db
                .add_mention(&episode.id, &record.id, "mentioned", entity.confidence)?;
            entity_records.insert(normalize_text(&record.canonical_name), record);
        }

        let mut indexed_docs: Vec<(String, String, String, String)> = Vec::new();
        indexed_docs.push((
            episode.id.clone(),
            "episode".to_string(),
            episode.layer.as_str().to_string(),
            episode.content.clone(),
        ));

        for record in entity_records.values() {
            indexed_docs.push((
                record.id.clone(),
                "entity".to_string(),
                record.layer.as_str().to_string(),
                entity_text_for_ranking(record),
            ));
        }

        for fact in facts {
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
                    Some(&episode.id),
                    vector.as_deref(),
                )?;
                entity_records.insert(subject_key.clone(), record.clone());
                indexed_docs.push((
                    record.id.clone(),
                    "entity".to_string(),
                    record.layer.as_str().to_string(),
                    entity_text_for_ranking(&record),
                ));
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
                    Some(&episode.id),
                    vector.as_deref(),
                )?;
                entity_records.insert(object_key.clone(), record.clone());
                indexed_docs.push((
                    record.id.clone(),
                    "entity".to_string(),
                    record.layer.as_str().to_string(),
                    entity_text_for_ranking(&record),
                ));
                record
            };

            let vector = self.embed_if_available(&format!(
                "{} {} {}",
                fact.subject, fact.predicate, fact.object
            ))?;
            let fact_record = self.db.insert_fact(
                &fact,
                input.layer,
                Some(&episode.id),
                Some(&subject_record.id),
                Some(&object_record.id),
                vector.as_deref(),
            )?;
            let _ = self.db.insert_edge(
                &subject_record.id,
                &fact.predicate,
                &object_record.id,
                fact.confidence,
                input.layer,
                Some(&episode.id),
            )?;
            indexed_docs.push((
                fact_record.id.clone(),
                "fact".to_string(),
                fact_record.layer.as_str().to_string(),
                fact_text_for_ranking(&fact_record),
            ));
        }

        self.upsert_text_documents(&indexed_docs)?;
        self.upsert_vector_documents_for_ids(&indexed_docs)?;
        self.refresh_l3_cache()?;
        self.refresh_session_cache(&episode.id, &input.content, entity_records.values())?;

        Ok(episode.id)
    }

    pub fn query(&self, request: RetrieveRequest) -> Result<QueryResultSet> {
        let started = Instant::now();
        let normalized = normalize_text(&request.query);
        let mut candidates: HashMap<String, Candidate> = HashMap::new();
        let limit = request.limit.max(1);
        let text_limit = if request.deep { limit * 12 } else { limit * 6 };
        let graph_limit = if request.deep { limit * 8 } else { limit * 4 };
        let graph_hops = if request.deep { 2 } else { 1 };

        if let Some(candidate) = self.l0_match(&normalized)? {
            add_candidate(&mut candidates, candidate);
        }

        for candidate in self.l3_matches(&normalized)? {
            add_candidate(&mut candidates, candidate);
        }

        for record in self.db.search_exact_alias(&request.query)? {
            let reason = match &record {
                MemoryRecord::Entity(_) => RetrieveReason::Alias,
                _ => RetrieveReason::Exact,
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
                            reasons: vec![RetrieveReason::Bm25],
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
                            reasons: vec![RetrieveReason::Vector],
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
                    reasons: vec![RetrieveReason::GraphHop { hops }],
                },
            );
        }

        let mut scored: Vec<Candidate> = candidates
            .into_values()
            .map(|mut candidate| {
                let recency = recency_boost(candidate.memory.updated_at());
                if recency > 0.0 {
                    candidate.score += recency;
                    candidate.reasons.push(RetrieveReason::RecencyBoost);
                }
                let layer_boost = candidate.memory.layer().boost();
                if layer_boost > 0.0 {
                    candidate.score += layer_boost;
                    candidate.reasons.push(RetrieveReason::LayerBoost);
                }
                let frequency_boost = hit_frequency_boost(candidate.memory.hit_count());
                if frequency_boost > 0.0 {
                    candidate.score += frequency_boost;
                    candidate.reasons.push(RetrieveReason::HitFrequencyBoost);
                }
                candidate
            })
            .collect();
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));

        let selected = mmr_select(scored, limit);
        let results = selected
            .into_iter()
            .map(|mut candidate| {
                candidate.reasons.push(RetrieveReason::MmrSelected);
                RetrieveResult {
                    memory: candidate.memory,
                    score: candidate.score,
                    reasons: candidate.reasons,
                }
            })
            .collect::<Vec<_>>();

        for result in &results {
            let _ = self.db.increment_hit_count(&result.memory);
        }
        self.record_query_session(&normalized, &results)?;

        debug!(
            query = %request.query,
            deep = request.deep,
            candidates = results.len(),
            elapsed_ms = started.elapsed().as_millis(),
            "memory query completed"
        );

        Ok(QueryResultSet {
            total_candidates: results.len(),
            deep_search_used: request.deep,
            results,
        })
    }

    pub fn inspect_memory(&self, id: &str) -> Result<MemoryRecord> {
        self.db
            .get_memory(id)?
            .with_context(|| format!("memory not found: {}", id))
    }

    pub fn consolidate(&self, trigger: ConsolidationTrigger) -> Result<ConsolidationReport> {
        self.db.create_consolidation_job(trigger.as_str())?;
        let mut report = ConsolidationReport {
            trigger: trigger.as_str().to_string(),
            jobs_created: 1,
            ..Default::default()
        };

        for group in self.db.duplicate_l1_episode_groups()? {
            if let Some(primary) = group.first() {
                self.db.update_layer("episode", primary, MemoryLayer::L2)?;
                report.promoted_to_l2 += 1;
                for duplicate in group.iter().skip(1) {
                    self.db.archive_record("episode", duplicate)?;
                    report.archived_records += 1;
                }
            }
        }

        for episode_id in self.db.eligible_episode_ids_for_l2()? {
            self.db
                .update_layer("episode", &episode_id, MemoryLayer::L2)?;
            report.promoted_to_l2 += 1;
        }

        for kind in ["episode", "entity", "fact"] {
            for id in self.db.eligible_ids_for_l3(kind)? {
                self.db.update_layer(kind, &id, MemoryLayer::L3)?;
                report.promoted_to_l3 += 1;
            }
        }

        let _ = self.db.complete_consolidation_jobs()?;
        self.refresh_l3_cache()?;
        Ok(report)
    }

    pub fn rebuild_indexes(&self, scope: RebuildScope) -> Result<RebuildReport> {
        let mut report = RebuildReport::default();

        if matches!(scope, RebuildScope::All | RebuildScope::Text) {
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

        if matches!(scope, RebuildScope::All | RebuildScope::Vector) {
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

    pub fn stats(&self) -> Result<EngineStats> {
        let (episode_count, entity_count, fact_count, edge_count) = self.db.stats()?;
        Ok(EngineStats {
            episode_count,
            entity_count,
            fact_count,
            edge_count,
            l3_cached: self.l3_cache.lock().expect("l3 mutex poisoned").len(),
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
            reasons: vec![RetrieveReason::L0],
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
                    reasons: vec![RetrieveReason::L3],
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

    fn record_query_session(
        &self,
        normalized_query: &str,
        results: &[RetrieveResult],
    ) -> Result<()> {
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

    fn upsert_text_documents(&self, docs: &[(String, String, String, String)]) -> Result<()> {
        let mut text_index = self.text_index.lock().expect("tantivy mutex poisoned");
        for (id, kind, layer, body) in docs {
            text_index.upsert_document(id, kind, layer, body)?;
        }
        self.db.record_index_state(
            "text",
            self.db.load_search_documents()?.len(),
            "ready",
            None,
        )?;
        Ok(())
    }

    fn upsert_vector_documents_for_ids(
        &self,
        docs: &[(String, String, String, String)],
    ) -> Result<()> {
        if self.config.embedding_provider.is_none() {
            return Ok(());
        }
        let mut vector_index = self.vector_index.lock().expect("vector mutex poisoned");
        let vectors = self.db.load_vector_documents()?;
        for (id, kind, _, _) in docs {
            if let Some((_, _, vector)) = vectors
                .iter()
                .into_iter()
                .find(|(doc_id, doc_kind, _)| doc_id == id && doc_kind == kind)
            {
                vector_index.upsert(kind, id, vector)?;
            }
        }
        self.db
            .record_index_state("vector", vectors.len(), "ready", None)?;
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

fn recency_boost(updated_at: chrono::DateTime<Utc>) -> f32 {
    let age_days = (Utc::now() - updated_at).num_days().max(0) as f32;
    (-(age_days / 30.0)).exp() * 0.18
}

fn hit_frequency_boost(hit_count: u64) -> f32 {
    ((hit_count as f32) + 1.0).ln() * 0.05
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

fn entity_text_for_ranking(entity: &EntityRecord) -> String {
    format!("{} {}", entity.canonical_name, entity.aliases.join(" "))
}

fn fact_text_for_ranking(fact: &crate::types::FactRecord) -> String {
    format!(
        "{} {} {}",
        fact.subject_text, fact.predicate, fact.object_text
    )
}

use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use anyhow::{Context, Result};
use tracing::debug;

use crate::{
    db::normalize_text,
    types::{
        EntityRecord, MemoryRecord, RecallReason, RecallRequest, RecallResult, RecallResultSet,
    },
};

use super::{Candidate, MemoryEngine, SessionCache};

enum RecallSearchStrategy {
    Fast,
    Deep,
}

impl MemoryEngine {
    pub fn recall(&self, request: RecallRequest) -> Result<RecallResultSet> {
        let started = Instant::now();
        let normalized = normalize_text(&request.query);
        let mut result = self.execute_query(&request, request.deep)?;
        if !request.deep
            && matches!(
                select_recall_search_strategy(&result),
                RecallSearchStrategy::Deep
            )
        {
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
                if let Some(memory) = self.db.get_active_memory_by_kind(&hit.kind, &hit.id)? {
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
            let has_vector_documents = self
                .vector_index
                .lock()
                .expect("vector mutex poisoned")
                .has_documents();
            if has_vector_documents {
                let query_vector = provider.embed_text(&request.query)?;
                let vector_index = self.vector_index.lock().expect("vector mutex poisoned");
                for hit in vector_index.search(&query_vector, text_limit)? {
                    if let Some(memory) = self.db.get_active_memory_by_kind(&hit.kind, &hit.id)? {
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
        let memories = results
            .iter()
            .map(|result| result.memory.clone())
            .collect::<Vec<_>>();
        let _ = self.db.increment_hit_counts(&memories);
        self.record_query_session(normalized_query, results)
    }

    fn l0_match(&self, normalized_query: &str) -> Result<Option<Candidate>> {
        let session = self.session.lock().expect("session mutex poisoned");
        let Some(entity_id) = session.recent_aliases.get(normalized_query).cloned() else {
            return Ok(None);
        };
        drop(session);
        let memory = self
            .db
            .get_active_memory(&entity_id)?
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
            if !record.is_active() {
                continue;
            }
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

    pub(super) fn refresh_session_cache<'a>(
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
        trim_session_cache(&mut session);
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
        trim_session_cache(&mut session);
        Ok(())
    }
}

fn trim_session_cache(session: &mut SessionCache) {
    if session.recent_memory_ids.len() > 128 {
        session.recent_memory_ids.drain(..64);
    }
    if session.recent_topics.len() > 64 {
        session.recent_topics.drain(..32);
    }
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

fn recency_boost(updated_at: chrono::DateTime<chrono::Utc>) -> f32 {
    let age_days = (chrono::Utc::now() - updated_at).num_days().max(0) as f32;
    (-(age_days / 30.0)).exp() * 0.18
}

fn hit_frequency_boost(hit_count: u64) -> f32 {
    ((hit_count as f32) + 1.0).ln() * 0.05
}

#[cfg(test)]
fn should_auto_escalate_to_deep_search(result: &RecallResultSet) -> bool {
    matches!(
        select_recall_search_strategy(result),
        RecallSearchStrategy::Deep
    )
}

fn select_recall_search_strategy(result: &RecallResultSet) -> RecallSearchStrategy {
    const WEAK_SINGLE_RESULT_SCORE_THRESHOLD: f32 = 0.9;
    const AMBIGUOUS_SCORE_GAP_THRESHOLD: f32 = 0.25;

    let Some(first) = result.results.first() else {
        return RecallSearchStrategy::Deep;
    };
    if has_decisive_reason(&first.reasons) {
        return RecallSearchStrategy::Fast;
    }

    if result.results.len() == 1 {
        return if first.score <= WEAK_SINGLE_RESULT_SCORE_THRESHOLD {
            RecallSearchStrategy::Deep
        } else {
            RecallSearchStrategy::Fast
        };
    }

    let second = &result.results[1];
    let score_gap = (first.score - second.score).abs();
    if score_gap <= AMBIGUOUS_SCORE_GAP_THRESHOLD {
        RecallSearchStrategy::Deep
    } else {
        RecallSearchStrategy::Fast
    }
}

fn has_decisive_reason(reasons: &[RecallReason]) -> bool {
    reasons.iter().any(|reason| {
        matches!(
            reason,
            RecallReason::L0 | RecallReason::L3 | RecallReason::Exact | RecallReason::Alias
        )
    })
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

    use crate::types::{EpisodeRecord, MemoryLayer, MemoryRecord, RecallResult, RecallResultSet};

    use super::{should_auto_escalate_to_deep_search, trim_session_cache};
    use crate::engine::SessionCache;

    fn episode_result(score: f32, reasons: Vec<crate::types::RecallReason>) -> RecallResult {
        RecallResult {
            memory: MemoryRecord::Episode(EpisodeRecord {
                id: "episode-1".to_string(),
                content: "Paris travel checklist for May.".to_string(),
                layer: MemoryLayer::L1,
                confidence: 0.9,
                source_episode_id: None,
                session_id: None,
                created_at: Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap(),
                last_seen_at: Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap(),
                archived_at: None,
                invalidated_at: None,
                hit_count: 0,
            }),
            score,
            reasons,
        }
    }

    #[test]
    fn weak_single_candidate_can_trigger_deep_search() {
        let result = RecallResultSet {
            deep_search_used: false,
            total_candidates: 1,
            results: vec![episode_result(0.72, vec![crate::types::RecallReason::Bm25])],
        };

        assert!(should_auto_escalate_to_deep_search(&result));
    }

    #[test]
    fn decisive_exact_hit_stays_on_fast_path() {
        let result = RecallResultSet {
            deep_search_used: false,
            total_candidates: 1,
            results: vec![episode_result(3.2, vec![crate::types::RecallReason::Exact])],
        };

        assert!(!should_auto_escalate_to_deep_search(&result));
    }

    #[test]
    fn trim_session_cache_caps_recent_topics_and_memory_ids() {
        let mut session = SessionCache::default();
        session.recent_memory_ids = (0..130).map(|index| format!("memory-{index}")).collect();
        session.recent_topics = (0..70).map(|index| format!("topic-{index}")).collect();

        trim_session_cache(&mut session);

        assert_eq!(session.recent_memory_ids.len(), 66);
        assert_eq!(session.recent_topics.len(), 38);
        assert_eq!(
            session.recent_memory_ids.first().map(String::as_str),
            Some("memory-64")
        );
        assert_eq!(
            session.recent_topics.first().map(String::as_str),
            Some("topic-32")
        );
    }
}

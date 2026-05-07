use super::{ranking::*, *};

impl MemoryEngine {
    pub(super) fn execute_query(
        &self,
        request: &RecallRequest,
        deep: bool,
    ) -> Result<RecallResultSet> {
        let mut candidates: HashMap<String, Candidate> = HashMap::new();
        let limit = request.limit.max(1);
        let text_limit = if deep { limit * 12 } else { limit * 6 };
        let graph_limit = if deep { limit * 8 } else { limit * 4 };
        let graph_hops = if deep { 2 } else { 1 };
        let normalized = normalize_text(&request.query);
        let active_subjects = self.active_working_subjects();
        let recent_memory_ids = self.recent_working_memory_ids();

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
                match provider.embed_text(&request.query) {
                    Ok(query_vector) => {
                        let vector_index = self.vector_index.lock().expect("vector mutex poisoned");
                        for hit in vector_index.search(&query_vector, text_limit)? {
                            if let Some(memory) =
                                self.db.get_active_memory_by_kind(&hit.kind, &hit.id)?
                            {
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
                    Err(error) => {
                        warn!(
                            error = %error,
                            "query embedding failed during recall; falling back to non-vector paths"
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
                let working_boost = working_set_boost(
                    &candidate.memory,
                    &active_subjects,
                    &recent_memory_ids,
                    &request.query,
                );
                if working_boost > 0.0 {
                    candidate.score += working_boost;
                    candidate.reasons.push(RecallReason::WorkingSet);
                }
                let layer_boost = gated_layer_boost(&request.query, &candidate.memory);
                if layer_boost > 0.0 {
                    candidate.score += layer_boost;
                    candidate.reasons.push(RecallReason::LayerBoost);
                }
                let frequency_boost = hit_frequency_boost(candidate.memory.hit_count());
                if frequency_boost > 0.0 {
                    candidate.score += frequency_boost;
                    candidate.reasons.push(RecallReason::HitFrequencyBoost);
                }
                candidate.score += answer_shape_boost(&request.query, &candidate.memory);
                candidate.score += subject_coverage_boost(&request.query, &candidate.memory);
                if has_subject_mismatch(&request.query, &candidate.memory) {
                    candidate.reasons.push(RecallReason::SubjectMismatch);
                }
                candidate
            })
            .collect();
        self.apply_rerank(deep, &request.query, limit, &mut scored)?;
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        if deep {
            filter_candidates_by_query_coverage(&request.query, &mut scored);
        }
        let expand_graph_records = request.include_related_records;
        dedupe_candidates_by_source(&mut scored, expand_graph_records, expand_graph_records);
        if !expand_graph_records && limit <= 5 {
            truncate_weak_small_limit_tail(&mut scored);
        }

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
        let reranked = match provider.rerank(query, &documents) {
            Ok(reranked) => reranked,
            Err(error) => {
                warn!(
                    error = %error,
                    "rerank provider failed during deep recall; keeping fused candidate order"
                );
                return Ok(());
            }
        };

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
    fn active_working_subjects(&self) -> Vec<String> {
        self.session
            .lock()
            .expect("session mutex poisoned")
            .active_subjects
            .clone()
    }
    fn recent_working_memory_ids(&self) -> Vec<String> {
        self.session
            .lock()
            .expect("session mutex poisoned")
            .recent_memory_ids
            .clone()
    }
}

fn working_set_boost(
    memory: &MemoryRecord,
    active_subjects: &[String],
    recent_memory_ids: &[String],
    query: &str,
) -> f32 {
    let mut boost: f32 = 0.0;
    if recent_memory_ids
        .iter()
        .rev()
        .take(16)
        .any(|id| id == memory.id())
    {
        boost = boost.max(0.22);
    }
    let query_subjects = query_subject_tokens(query);
    for subject in active_subjects.iter().rev().take(12) {
        if !query_subjects.is_empty() && !query_subjects.contains(subject) {
            continue;
        }
        if memory_contains_subject(memory, subject) {
            boost = boost.max(0.30);
        }
    }
    boost
}

fn gated_layer_boost(query: &str, memory: &MemoryRecord) -> f32 {
    let base = memory.layer().boost();
    if memory.layer() != crate::types::MemoryLayer::L3 {
        return base;
    }
    if has_subject_mismatch(query, memory) || query_coverage(query, memory) < 0.25 {
        return 0.0;
    }
    base
}

fn has_subject_mismatch(query: &str, memory: &MemoryRecord) -> bool {
    let subjects = query_subject_tokens(query);
    !subjects.is_empty()
        && !subjects
            .iter()
            .any(|subject| memory_contains_subject(memory, subject))
}

fn truncate_weak_small_limit_tail(candidates: &mut Vec<Candidate>) {
    if candidates.len() < 2 {
        return;
    }
    candidates.sort_by(|left, right| right.score.total_cmp(&left.score));
    let top_score = candidates[0].score;
    if top_score < 1.8 {
        return;
    }
    let min_score = top_score - 0.85;
    candidates.retain(|candidate| candidate.score >= min_score);
}

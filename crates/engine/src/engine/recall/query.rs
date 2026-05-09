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
        let active_subjects = self.active_working_subjects()?;
        let recent_memory_ids = self.recent_working_memory_ids()?;

        if let Some(candidate) = self.session_cache_match(&normalized)? {
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

        for memory in self.working_set_candidates(&request.query, &recent_memory_ids)? {
            add_candidate(
                &mut candidates,
                Candidate {
                    memory,
                    score: 1.45,
                    reasons: vec![RecallReason::WorkingSet],
                },
            );
        }

        let mut scored = Vec::new();
        for mut candidate in candidates.into_values() {
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
            if self
                .db
                .is_pinned(candidate.memory.kind(), candidate.memory.id())?
            {
                let boost = pinned_boost(&request.query, &candidate.memory);
                if boost > 0.0 {
                    candidate.score += boost;
                    candidate.reasons.push(RecallReason::Pinned);
                }
            }
            candidate.score += answer_shape_boost(&request.query, &candidate.memory);
            candidate.score += subject_coverage_boost(&request.query, &candidate.memory);
            if has_subject_mismatch(&request.query, &candidate.memory) {
                candidate.reasons.push(RecallReason::SubjectMismatch);
            }
            scored.push(candidate);
        }
        scored.sort_by(|a, b| b.score.total_cmp(&a.score));
        let total_candidates = scored.len();
        let capabilities = recall_capabilities(&scored);
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
            total_candidates,
            deep_search_used: deep,
            results,
            provider_calls: 0,
            capabilities,
        })
    }
    fn session_cache_match(&self, normalized_query: &str) -> Result<Option<Candidate>> {
        let session = self.session.lock().expect("session mutex poisoned");
        let Some(entity_id) = session.recent_aliases.get(normalized_query).cloned() else {
            return Ok(None);
        };
        drop(session);
        let memory = self
            .db
            .get_active_memory(&entity_id)?
            .with_context(|| format!("dangling session cache entity reference: {}", entity_id))?;
        Ok(Some(Candidate {
            memory,
            score: 3.5,
            reasons: vec![RecallReason::SessionCache],
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
    fn active_working_subjects(&self) -> Result<Vec<String>> {
        let mut subjects = self
            .session
            .lock()
            .expect("session mutex poisoned")
            .active_subjects
            .clone();
        let mut seen = subjects.iter().cloned().collect::<HashSet<_>>();
        for subject in self.db.recent_working_set_subjects(32)? {
            if seen.insert(subject.clone()) {
                subjects.push(subject);
            }
        }
        Ok(subjects)
    }
    fn recent_working_memory_ids(&self) -> Result<Vec<String>> {
        let mut ids = self
            .session
            .lock()
            .expect("session mutex poisoned")
            .recent_memory_ids
            .clone();
        let mut seen = ids.iter().cloned().collect::<HashSet<_>>();
        for id in self.db.recent_working_set_memory_ids(32)? {
            if seen.insert(id.clone()) {
                ids.push(id);
            }
        }
        Ok(ids)
    }
    fn working_set_candidates(
        &self,
        query: &str,
        recent_memory_ids: &[String],
    ) -> Result<Vec<MemoryRecord>> {
        if recent_memory_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut result = Vec::new();
        for id in recent_memory_ids.iter().rev().take(16) {
            let Some(memory) = self.db.get_active_memory(id)? else {
                continue;
            };
            if query_coverage(query, &memory) >= 0.5 {
                result.push(memory);
            }
        }
        Ok(result)
    }
}

fn recall_capabilities(candidates: &[Candidate]) -> RecallCapabilities {
    let mut capabilities = RecallCapabilities {
        text: false,
        vector: false,
        l1: false,
        l2: false,
        l3: false,
        working_set: false,
    };

    for candidate in candidates {
        if candidate
            .reasons
            .iter()
            .any(|reason| matches!(reason, RecallReason::Bm25))
        {
            capabilities.text = true;
        }
        if candidate
            .reasons
            .iter()
            .any(|reason| matches!(reason, RecallReason::Vector))
        {
            capabilities.vector = true;
        }
        if candidate
            .reasons
            .iter()
            .any(|reason| matches!(reason, RecallReason::WorkingSet))
        {
            capabilities.working_set = true;
        }
        match candidate.memory.layer() {
            crate::types::MemoryLayer::L1 => capabilities.l1 = true,
            crate::types::MemoryLayer::L2 => capabilities.l2 = true,
            crate::types::MemoryLayer::L3 => capabilities.l3 = true,
        }
    }

    capabilities
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
        if query_subjects.len() > 2 && !query_subjects.contains(subject) {
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

use super::{ranking::query_subject_tokens, session_cache::trim_session_cache, *};

impl MemoryEngine {
    pub(super) fn commit_query_results(&self, query: &str, results: &[RecallResult]) -> Result<()> {
        let normalized_query = normalize_text(query);
        let memories = results
            .iter()
            .filter(|result| has_retrieval_evidence(result))
            .map(|result| result.memory.clone())
            .collect::<Vec<_>>();
        let _ = self.db.increment_hit_counts(&memories);
        self.db.mark_working_set_records(&memories)?;
        self.record_query_session(query, &normalized_query, results)
    }
    pub(in crate::engine) fn refresh_session_cache<'a>(
        &self,
        episode_id: &str,
        content: &str,
        entities: impl Iterator<Item = &'a EntityRecord>,
    ) -> Result<()> {
        let mut session = self.session.lock().expect("session mutex poisoned");
        session.recent_memory_ids.push(episode_id.to_string());
        session.recent_topics.push(normalize_text(content));
        for subject in query_subject_tokens(content) {
            push_unique_recent(&mut session.active_subjects, subject);
        }
        for entity in entities {
            push_unique_recent(
                &mut session.active_subjects,
                normalize_text(&entity.canonical_name),
            );
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
    fn record_query_session(
        &self,
        query: &str,
        normalized_query: &str,
        results: &[RecallResult],
    ) -> Result<()> {
        let mut session = self.session.lock().expect("session mutex poisoned");
        session.recent_topics.push(normalized_query.to_string());
        for subject in query_subject_tokens(query) {
            push_unique_recent(&mut session.active_subjects, subject);
        }
        for result in results {
            session
                .recent_memory_ids
                .push(result.memory.id().to_string());
            for subject in query_subject_tokens(&result.memory.text_for_ranking()) {
                push_unique_recent(&mut session.active_subjects, subject);
            }
            if let MemoryRecord::Entity(entity) = &result.memory {
                push_unique_recent(
                    &mut session.active_subjects,
                    normalize_text(&entity.canonical_name),
                );
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

fn has_retrieval_evidence(result: &RecallResult) -> bool {
    result.reasons.iter().any(|reason| {
        matches!(
            reason,
            RecallReason::L0
                | RecallReason::L3
                | RecallReason::Exact
                | RecallReason::Alias
                | RecallReason::Bm25
                | RecallReason::Vector
                | RecallReason::GraphHop { .. }
        )
    })
}

fn push_unique_recent(target: &mut Vec<String>, value: String) {
    if value.is_empty() {
        return;
    }
    target.retain(|existing| existing != &value);
    target.push(value);
}

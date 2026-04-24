use super::{session_cache::trim_session_cache, *};

impl MemoryEngine {
    pub(super) fn commit_query_results(
        &self,
        normalized_query: &str,
        results: &[RecallResult],
    ) -> Result<()> {
        let memories = results
            .iter()
            .map(|result| result.memory.clone())
            .collect::<Vec<_>>();
        let _ = self.db.increment_hit_counts(&memories);
        self.record_query_session(normalized_query, results)
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

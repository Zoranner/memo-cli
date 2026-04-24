use std::{
    collections::{HashMap, HashSet},
    time::Instant,
};

use anyhow::{Context, Result};
use tracing::{debug, warn};

use crate::{
    db::normalize_text,
    types::{
        EntityRecord, MemoryRecord, RecallReason, RecallRequest, RecallResult, RecallResultSet,
    },
};

use self::strategy::select_recall_search_strategy;
use super::{Candidate, MemoryEngine, SessionCache};

mod query;
mod ranking;
mod session;
mod session_cache;
mod strategy;
#[cfg(test)]
mod tests;

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
}

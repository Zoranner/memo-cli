use serde::{Deserialize, Serialize};

use super::MemoryRecord;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecallReason {
    L0,
    L3,
    Exact,
    Alias,
    Bm25,
    Vector,
    Rerank,
    GraphHop { hops: usize },
    RecencyBoost,
    LayerBoost,
    HitFrequencyBoost,
    MmrSelected,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResult {
    pub memory: MemoryRecord,
    pub score: f32,
    #[serde(default)]
    pub reasons: Vec<RecallReason>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallResultSet {
    pub results: Vec<RecallResult>,
    pub deep_search_used: bool,
    pub total_candidates: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecallRequest {
    pub query: String,
    #[serde(default = "default_limit")]
    pub limit: usize,
    #[serde(default)]
    pub deep: bool,
}

fn default_limit() -> usize {
    10
}

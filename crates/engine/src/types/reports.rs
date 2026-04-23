use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DreamTrigger {
    SessionEnd,
    Idle,
    BeforeCompaction,
    Manual,
}

impl DreamTrigger {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SessionEnd => "session_end",
            Self::Idle => "idle",
            Self::BeforeCompaction => "before_compaction",
            Self::Manual => "manual",
        }
    }
}

impl std::str::FromStr for DreamTrigger {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "session_end" => Ok(Self::SessionEnd),
            "idle" => Ok(Self::Idle),
            "before_compaction" => Ok(Self::BeforeCompaction),
            "manual" => Ok(Self::Manual),
            _ => anyhow::bail!("invalid dream trigger: {}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DreamReport {
    pub trigger: String,
    pub passes_run: usize,
    pub structured_episodes: usize,
    pub structured_entities: usize,
    pub structured_facts: usize,
    pub extraction_failures: usize,
    pub promoted_to_l2: usize,
    pub promoted_to_l3: usize,
    pub downgraded_records: usize,
    pub archived_records: usize,
    pub invalidated_records: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum RestoreScope {
    All,
    Text,
    Vector,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RestoreReport {
    pub text_documents: usize,
    pub vector_documents: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IndexStatus {
    pub name: String,
    pub doc_count: usize,
    pub status: String,
    pub detail: Option<String>,
    pub pending_updates: usize,
    pub failed_updates: usize,
    pub failed_attempts_max: usize,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LayerSummary {
    pub l1: usize,
    pub l2: usize,
    pub l3: usize,
    pub archived: usize,
    pub invalidated: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemState {
    pub episode_count: usize,
    pub entity_count: usize,
    pub fact_count: usize,
    pub edge_count: usize,
    pub layers: LayerSummary,
    pub l3_cached: usize,
    pub text_index: IndexStatus,
    pub vector_index: IndexStatus,
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::DreamTrigger;

    #[test]
    fn invalid_dream_trigger_uses_dream_wording() {
        let error = DreamTrigger::from_str("nope").expect_err("expected invalid trigger");
        assert!(error.to_string().contains("invalid dream trigger"));
    }
}

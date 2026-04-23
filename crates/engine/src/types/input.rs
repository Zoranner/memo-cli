use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::MemoryLayer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ExtractionSource {
    Manual,
    Provider,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityInput {
    pub entity_type: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default = "default_extraction_source")]
    pub source: ExtractionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactInput {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
    #[serde(default = "default_extraction_source")]
    pub source: ExtractionSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeInput {
    pub content: String,
    #[serde(default = "default_l1")]
    pub layer: MemoryLayer,
    #[serde(default)]
    pub entities: Vec<EntityInput>,
    #[serde(default)]
    pub facts: Vec<FactInput>,
    pub source_episode_id: Option<String>,
    #[serde(default)]
    pub session_id: Option<String>,
    #[serde(default)]
    pub recorded_at: Option<DateTime<Utc>>,
    #[serde(default = "default_confidence")]
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RememberPreview {
    pub content: String,
    pub layer: MemoryLayer,
    pub entities: Vec<EntityInput>,
    pub facts: Vec<FactInput>,
    pub source_episode_id: Option<String>,
    pub session_id: Option<String>,
    pub recorded_at: Option<DateTime<Utc>>,
    pub confidence: f32,
}

fn default_l1() -> MemoryLayer {
    MemoryLayer::L1
}

fn default_confidence() -> f32 {
    0.85
}

fn default_extraction_source() -> ExtractionSource {
    ExtractionSource::Manual
}

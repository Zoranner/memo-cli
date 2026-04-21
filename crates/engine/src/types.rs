use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{EmbeddingProvider, ExtractionProvider, RerankProvider};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MemoryLayer {
    L1,
    L2,
    L3,
}

impl MemoryLayer {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
        }
    }

    pub fn boost(self) -> f32 {
        match self {
            Self::L1 => 0.0,
            Self::L2 => 0.12,
            Self::L3 => 0.25,
        }
    }
}

impl std::str::FromStr for MemoryLayer {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_uppercase().as_str() {
            "L1" => Ok(Self::L1),
            "L2" => Ok(Self::L2),
            "L3" => Ok(Self::L3),
            _ => anyhow::bail!("invalid memory layer: {}", s),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum LayerState {
    Active,
    Archived,
    Invalidated,
}

impl LayerState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
            Self::Invalidated => "invalidated",
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub vector_dimension: usize,
    pub l3_cache_limit: usize,
    #[serde(skip)]
    pub embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    #[serde(skip)]
    pub rerank_provider: Option<Arc<dyn RerankProvider>>,
    #[serde(skip)]
    pub extraction_provider: Option<Arc<dyn ExtractionProvider>>,
}

impl EngineConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            vector_dimension: 384,
            l3_cache_limit: 256,
            embedding_provider: None,
            rerank_provider: None,
            extraction_provider: None,
        }
    }

    pub fn sqlite_path(&self) -> PathBuf {
        self.data_dir.join("memory.db")
    }

    pub fn text_index_dir(&self) -> PathBuf {
        self.data_dir.join("text-index")
    }

    pub fn vector_index_path(&self) -> PathBuf {
        self.data_dir.join("vector-index.json")
    }

    pub fn with_embedding_provider(mut self, provider: Arc<dyn EmbeddingProvider>) -> Self {
        self.vector_dimension = provider.dimension();
        self.embedding_provider = Some(provider);
        self
    }

    pub fn with_rerank_provider(mut self, provider: Arc<dyn RerankProvider>) -> Self {
        self.rerank_provider = Some(provider);
        self
    }

    pub fn with_extraction_provider(mut self, provider: Arc<dyn ExtractionProvider>) -> Self {
        self.extraction_provider = Some(provider);
        self
    }

    pub fn ensure_dirs(&self) -> anyhow::Result<()> {
        std::fs::create_dir_all(&self.data_dir)?;
        std::fs::create_dir_all(self.text_index_dir())?;
        Ok(())
    }

    pub fn data_dir(&self) -> &Path {
        &self.data_dir
    }
}

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EpisodeRecord {
    pub id: String,
    pub content: String,
    pub layer: MemoryLayer,
    pub confidence: f32,
    pub source_episode_id: Option<String>,
    pub session_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityRecord {
    pub id: String,
    pub entity_type: String,
    pub canonical_name: String,
    pub aliases: Vec<String>,
    pub layer: MemoryLayer,
    pub confidence: f32,
    pub source_episode_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub last_seen_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FactRecord {
    pub id: String,
    pub subject_entity_id: Option<String>,
    pub subject_text: String,
    pub predicate: String,
    pub object_entity_id: Option<String>,
    pub object_text: String,
    pub layer: MemoryLayer,
    pub confidence: f32,
    pub source_episode_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgeRecord {
    pub id: String,
    pub subject_entity_id: String,
    pub predicate: String,
    pub object_entity_id: String,
    pub weight: f32,
    pub source_episode_id: Option<String>,
    pub layer: MemoryLayer,
    pub valid_from: Option<DateTime<Utc>>,
    pub valid_to: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub archived_at: Option<DateTime<Utc>>,
    pub invalidated_at: Option<DateTime<Utc>>,
    pub hit_count: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MemoryRecord {
    Episode(EpisodeRecord),
    Entity(EntityRecord),
    Fact(FactRecord),
    Edge(EdgeRecord),
}

impl MemoryRecord {
    pub fn id(&self) -> &str {
        match self {
            Self::Episode(record) => &record.id,
            Self::Entity(record) => &record.id,
            Self::Fact(record) => &record.id,
            Self::Edge(record) => &record.id,
        }
    }

    pub fn kind(&self) -> &'static str {
        match self {
            Self::Episode(_) => "episode",
            Self::Entity(_) => "entity",
            Self::Fact(_) => "fact",
            Self::Edge(_) => "edge",
        }
    }

    pub fn layer(&self) -> MemoryLayer {
        match self {
            Self::Episode(record) => record.layer,
            Self::Entity(record) => record.layer,
            Self::Fact(record) => record.layer,
            Self::Edge(record) => record.layer,
        }
    }

    pub fn hit_count(&self) -> u64 {
        match self {
            Self::Episode(record) => record.hit_count,
            Self::Entity(record) => record.hit_count,
            Self::Fact(record) => record.hit_count,
            Self::Edge(record) => record.hit_count,
        }
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        match self {
            Self::Episode(record) => record.updated_at,
            Self::Entity(record) => record.updated_at,
            Self::Fact(record) => record.updated_at,
            Self::Edge(record) => record.updated_at,
        }
    }

    pub fn activity_at(&self) -> DateTime<Utc> {
        match self {
            Self::Episode(record) => record.last_seen_at,
            Self::Entity(record) => record.last_seen_at,
            Self::Fact(record) => record.updated_at,
            Self::Edge(record) => record.updated_at,
        }
    }

    pub fn text_for_ranking(&self) -> String {
        match self {
            Self::Episode(record) => record.content.clone(),
            Self::Entity(record) => {
                format!("{} {}", record.canonical_name, record.aliases.join(" "))
            }
            Self::Fact(record) => {
                format!(
                    "{} {} {}",
                    record.subject_text, record.predicate, record.object_text
                )
            }
            Self::Edge(record) => {
                format!(
                    "{} {} {}",
                    record.subject_entity_id, record.predicate, record.object_entity_id
                )
            }
        }
    }
}

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
            _ => anyhow::bail!("invalid consolidation trigger: {}", s),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DreamReport {
    pub trigger: String,
    pub promoted_to_l2: usize,
    pub promoted_to_l3: usize,
    pub downgraded_records: usize,
    pub archived_records: usize,
    pub invalidated_records: usize,
    pub jobs_created: usize,
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
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DreamJobStats {
    pub pending: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SystemState {
    pub episode_count: usize,
    pub entity_count: usize,
    pub fact_count: usize,
    pub edge_count: usize,
    pub l3_cached: usize,
    pub dream_jobs: DreamJobStats,
    pub text_index: IndexStatus,
    pub vector_index: IndexStatus,
}

fn default_l1() -> MemoryLayer {
    MemoryLayer::L1
}

fn default_limit() -> usize {
    10
}

fn default_confidence() -> f32 {
    0.85
}

fn default_extraction_source() -> ExtractionSource {
    ExtractionSource::Manual
}

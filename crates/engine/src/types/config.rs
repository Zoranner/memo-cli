use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

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

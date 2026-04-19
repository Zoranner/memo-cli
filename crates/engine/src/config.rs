use std::fmt;
use std::path::PathBuf;
use std::sync::Arc;

use memo_model_api::{EmbeddingProvider, ExtractionProvider, RerankProvider};

pub struct EngineConfig {
    pub data_dir: PathBuf,
    pub vector_dimension: usize,
    pub l3_cache_capacity: usize,
    pub embedding_provider: Option<Arc<dyn EmbeddingProvider>>,
    pub rerank_provider: Option<Arc<dyn RerankProvider>>,
    pub extraction_provider: Option<Arc<dyn ExtractionProvider>>,
}

impl EngineConfig {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
            vector_dimension: 384,
            l3_cache_capacity: 512,
            embedding_provider: None,
            rerank_provider: None,
            extraction_provider: None,
        }
    }

    pub fn database_path(&self) -> PathBuf {
        self.data_dir.join("memory.sqlite")
    }

    pub fn text_index_dir(&self) -> PathBuf {
        self.data_dir.join("text-index")
    }

    pub fn vector_index_path(&self) -> PathBuf {
        self.data_dir.join("vector-index.usearch")
    }

    pub fn with_vector_dimension(mut self, dimension: usize) -> Self {
        self.vector_dimension = dimension;
        self
    }

    pub fn with_l3_cache_capacity(mut self, capacity: usize) -> Self {
        self.l3_cache_capacity = capacity;
        self
    }

    pub fn with_embedding_provider(
        mut self,
        provider: Arc<dyn EmbeddingProvider>,
    ) -> Self {
        self.vector_dimension = provider.dimension();
        self.embedding_provider = Some(provider);
        self
    }

    pub fn with_rerank_provider(mut self, provider: Arc<dyn RerankProvider>) -> Self {
        self.rerank_provider = Some(provider);
        self
    }

    pub fn with_extraction_provider(
        mut self,
        provider: Arc<dyn ExtractionProvider>,
    ) -> Self {
        self.extraction_provider = Some(provider);
        self
    }
}

impl fmt::Debug for EngineConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("EngineConfig")
            .field("data_dir", &self.data_dir)
            .field("vector_dimension", &self.vector_dimension)
            .field("l3_cache_capacity", &self.l3_cache_capacity)
            .field(
                "embedding_provider",
                &self.embedding_provider.as_ref().map(|_| "configured"),
            )
            .field(
                "rerank_provider",
                &self.rerank_provider.as_ref().map(|_| "configured"),
            )
            .field(
                "extraction_provider",
                &self.extraction_provider.as_ref().map(|_| "configured"),
            )
            .finish()
    }
}

use anyhow::Result;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedEntity {
    pub entity_type: String,
    pub name: String,
    #[serde(default)]
    pub aliases: Vec<String>,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractedFact {
    pub subject: String,
    pub predicate: String,
    pub object: String,
    #[serde(default)]
    pub confidence: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct ExtractionResult {
    #[serde(default)]
    pub entities: Vec<ExtractedEntity>,
    #[serde(default)]
    pub facts: Vec<ExtractedFact>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RerankScore {
    pub index: usize,
    pub score: f32,
}

pub trait EmbeddingProvider: Send + Sync {
    fn dimension(&self) -> usize;
    fn embed_text(&self, text: &str) -> Result<Vec<f32>>;
}

pub trait RerankProvider: Send + Sync {
    fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<RerankScore>>;
}

pub trait ExtractionProvider: Send + Sync {
    fn extract(&self, text: &str) -> Result<ExtractionResult>;
}

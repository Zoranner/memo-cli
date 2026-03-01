//! Provider Traits

use anyhow::Result;
use async_trait::async_trait;

/// Embedding Provider Trait
#[async_trait]
pub trait EmbedProvider: Send + Sync {
    /// 编码单个文本
    async fn encode(&self, text: &str) -> Result<Vec<f32>>;

    /// 批量编码文本
    async fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>>;

    /// 获取向量维度
    fn dimension(&self) -> usize;
}

/// Rerank 结果项
#[derive(Debug, Clone)]
pub struct RerankItem {
    pub index: usize,
    pub score: f64,
}

/// Rerank Provider Trait
#[async_trait]
pub trait RerankProvider: Send + Sync {
    /// Rerank 文档
    async fn rerank(
        &self,
        query: &str,
        documents: &[&str],
        top_n: Option<usize>,
    ) -> Result<Vec<RerankItem>>;
}

//! 共享类型和 OpenAI 兼容格式实现

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::ResolvedService;

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

/// 文本规范化
pub(crate) fn normalize_for_embedding(text: &str) -> String {
    let text = text.trim();
    let re = regex::Regex::new(r"\s+").unwrap();
    re.replace_all(text, " ").to_string()
}

/// OpenAI 兼容格式 Embed（供阿里云、OpenAI、Ollama 使用）
pub struct OpenaiCompatibleEmbed {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    dimension: usize,
}

#[derive(Debug, Serialize)]
struct OpenaiEmbedRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OpenaiEmbedResponse {
    data: Vec<OpenaiEmbedData>,
}

#[derive(Debug, Deserialize)]
struct OpenaiEmbedData {
    embedding: Vec<f32>,
}

impl OpenaiCompatibleEmbed {
    pub fn new(config: &ResolvedService, dimension: usize) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;
        Ok(Self {
            client,
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url: config.base_url.clone(),
            dimension,
        })
    }
}

#[async_trait]
impl EmbedProvider for OpenaiCompatibleEmbed {
    async fn encode(&self, text: &str) -> Result<Vec<f32>> {
        let normalized = normalize_for_embedding(text);
        let embeddings = self.encode_batch(&[&normalized]).await?;
        Ok(embeddings.into_iter().next().unwrap())
    }

    async fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let normalized: Vec<String> = texts.iter().map(|t| normalize_for_embedding(t)).collect();

        let request = OpenaiEmbedRequest {
            model: self.model.clone(),
            input: normalized,
            dimensions: Some(self.dimension),
        };

        let url = format!("{}/embeddings", self.base_url);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.api_key))
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await?;
            tracing::error!("Embed API error ({}): {}", status, error_text);
            anyhow::bail!("Embed API error ({}): {}", status, error_text);
        }

        let embed_response: OpenaiEmbedResponse = response.json().await?;

        Ok(embed_response
            .data
            .into_iter()
            .map(|d| d.embedding)
            .collect())
    }

    fn dimension(&self) -> usize {
        self.dimension
    }
}

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

/// 创建 Embedding Provider
pub fn create_embed_provider(config: &ResolvedService) -> Result<Box<dyn EmbedProvider>> {
    let dimension = config
        .get_int("dimension")
        .ok_or_else(|| anyhow::anyhow!("Missing 'dimension' in embed service config"))?
        as usize;

    if config.base_url.contains("dashscope.aliyuncs.com")
        || config.base_url.contains("api.openai.com")
        || config.base_url.contains("localhost")
    {
        // OpenAI 兼容格式（阿里云、OpenAI、Ollama）
        Ok(Box::new(OpenAIEmbedProvider::new(config, dimension)?))
    } else if config.base_url.contains("bigmodel.cn") {
        // 智谱 AI
        Ok(Box::new(ZhipuEmbedProvider::new(config, dimension)?))
    } else {
        anyhow::bail!("Unknown embed provider for base_url: {}", config.base_url)
    }
}

/// 文本规范化
fn normalize_for_embedding(text: &str) -> String {
    // 移除多余的空白字符
    let text = text.trim();

    // 将连续的空白字符替换为单个空格
    let re = regex::Regex::new(r"\s+").unwrap();
    re.replace_all(text, " ").to_string()
}

// ===== OpenAI 兼容格式 Provider =====

pub struct OpenAIEmbedProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    dimension: usize,
}

#[derive(Debug, Serialize)]
struct OpenAIEmbedRequest {
    model: String,
    input: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dimensions: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbedResponse {
    data: Vec<OpenAIEmbedData>,
}

#[derive(Debug, Deserialize)]
struct OpenAIEmbedData {
    embedding: Vec<f32>,
}

impl OpenAIEmbedProvider {
    pub fn new(config: &ResolvedService, dimension: usize) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        tracing::info!(
            "Created OpenAIEmbedProvider: model={}, dimension={}, base_url={}",
            config.model,
            dimension,
            config.base_url
        );

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
impl EmbedProvider for OpenAIEmbedProvider {
    async fn encode(&self, text: &str) -> Result<Vec<f32>> {
        let normalized = normalize_for_embedding(text);
        let embeddings = self.encode_batch(&[&normalized]).await?;
        Ok(embeddings.into_iter().next().unwrap())
    }

    async fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let normalized: Vec<String> = texts.iter().map(|t| normalize_for_embedding(t)).collect();

        let request = OpenAIEmbedRequest {
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
            tracing::error!("OpenAI embed API error ({}): {}", status, error_text);
            anyhow::bail!("OpenAI embed API error ({}): {}", status, error_text);
        }

        let embed_response: OpenAIEmbedResponse = response.json().await?;

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

// ===== 智谱 AI Provider =====

pub struct ZhipuEmbedProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    dimension: usize,
}

#[derive(Debug, Serialize)]
struct ZhipuEmbedRequest {
    model: String,
    input: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ZhipuEmbedResponse {
    data: Vec<ZhipuEmbedData>,
}

#[derive(Debug, Deserialize)]
struct ZhipuEmbedData {
    embedding: Vec<f32>,
}

impl ZhipuEmbedProvider {
    pub fn new(config: &ResolvedService, dimension: usize) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(30)).build()?;

        tracing::info!(
            "Created ZhipuEmbedProvider: model={}, dimension={}, base_url={}",
            config.model,
            dimension,
            config.base_url
        );

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
impl EmbedProvider for ZhipuEmbedProvider {
    async fn encode(&self, text: &str) -> Result<Vec<f32>> {
        let normalized = normalize_for_embedding(text);
        let embeddings = self.encode_batch(&[&normalized]).await?;
        Ok(embeddings.into_iter().next().unwrap())
    }

    async fn encode_batch(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>> {
        let normalized: Vec<String> = texts.iter().map(|t| normalize_for_embedding(t)).collect();

        let request = ZhipuEmbedRequest {
            model: self.model.clone(),
            input: normalized,
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
            tracing::error!("Zhipu embed API error ({}): {}", status, error_text);
            anyhow::bail!("Zhipu embed API error ({}): {}", status, error_text);
        }

        let embed_response: ZhipuEmbedResponse = response.json().await?;

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

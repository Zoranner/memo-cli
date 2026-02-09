use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::ResolvedService;

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

/// 创建 Rerank Provider
pub fn create_rerank_provider(config: &ResolvedService) -> Result<Box<dyn RerankProvider>> {
    if config.base_url.contains("dashscope.aliyuncs.com") {
        Ok(Box::new(AliyunRerankProvider::new(config)?))
    } else if config.base_url.contains("bigmodel.cn") {
        Ok(Box::new(ZhipuRerankProvider::new(config)?))
    } else {
        anyhow::bail!(
            "Unknown rerank provider for base_url: {}. Supported: dashscope.aliyuncs.com, bigmodel.cn",
            config.base_url
        )
    }
}

// ===== 阿里云 Rerank Provider =====

pub struct AliyunRerankProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Debug, Serialize)]
struct AliyunRerankRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    top_n: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AliyunRerankResponse {
    results: Vec<AliyunRerankResult>,
}

#[derive(Debug, Deserialize)]
struct AliyunRerankResult {
    index: usize,
    relevance_score: f64,
}

impl AliyunRerankProvider {
    pub fn new(config: &ResolvedService) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

        tracing::info!(
            "Created AliyunRerankProvider: model={}, base_url={}",
            config.model,
            config.base_url
        );

        Ok(Self {
            client,
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url: config.base_url.clone(),
        })
    }
}

#[async_trait]
impl RerankProvider for AliyunRerankProvider {
    async fn rerank(
        &self,
        query: &str,
        documents: &[&str],
        top_n: Option<usize>,
    ) -> Result<Vec<RerankItem>> {
        tracing::debug!(
            "Aliyun reranking {} documents, top_n={:?}",
            documents.len(),
            top_n
        );

        let request = AliyunRerankRequest {
            model: self.model.clone(),
            query: query.to_string(),
            documents: documents.iter().map(|s| s.to_string()).collect(),
            top_n,
        };

        let url = format!("{}/reranks", self.base_url);

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
            tracing::error!("Aliyun rerank API error ({}): {}", status, error_text);
            anyhow::bail!("Aliyun rerank API error ({}): {}", status, error_text);
        }

        let rerank_response: AliyunRerankResponse = response.json().await?;

        tracing::debug!(
            "Aliyun rerank returned {} results",
            rerank_response.results.len()
        );
        for result in &rerank_response.results {
            tracing::debug!(
                "  Result: index={}, relevance_score={:.6}",
                result.index,
                result.relevance_score
            );
        }

        Ok(rerank_response
            .results
            .into_iter()
            .map(|r| RerankItem {
                index: r.index,
                score: r.relevance_score,
            })
            .collect())
    }
}

// ===== 智谱 Rerank Provider =====

pub struct ZhipuRerankProvider {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
}

#[derive(Debug, Serialize)]
struct ZhipuRerankRequest {
    model: String,
    query: String,
    documents: Vec<String>,
    top_n: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct ZhipuRerankResponse {
    results: Vec<ZhipuRerankResult>,
}

#[derive(Debug, Deserialize)]
struct ZhipuRerankResult {
    index: usize,
    relevance_score: f64,
}

impl ZhipuRerankProvider {
    pub fn new(config: &ResolvedService) -> Result<Self> {
        let client = Client::builder().timeout(Duration::from_secs(60)).build()?;

        tracing::warn!(
            "⚠️  ZhipuRerankProvider has known quality issues (all scores return ~1.0). Consider using Aliyun instead."
        );
        tracing::info!(
            "Created ZhipuRerankProvider: model={}, base_url={}",
            config.model,
            config.base_url
        );

        Ok(Self {
            client,
            api_key: config.api_key.clone(),
            model: config.model.clone(),
            base_url: config.base_url.clone(),
        })
    }
}

#[async_trait]
impl RerankProvider for ZhipuRerankProvider {
    async fn rerank(
        &self,
        query: &str,
        documents: &[&str],
        top_n: Option<usize>,
    ) -> Result<Vec<RerankItem>> {
        tracing::debug!(
            "Zhipu reranking {} documents, top_n={:?}",
            documents.len(),
            top_n
        );

        let request = ZhipuRerankRequest {
            model: self.model.clone(),
            query: query.to_string(),
            documents: documents.iter().map(|s| s.to_string()).collect(),
            top_n,
        };

        let url = format!("{}/rerank", self.base_url);

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
            tracing::error!("Zhipu rerank API error ({}): {}", status, error_text);
            anyhow::bail!("Zhipu rerank API error ({}): {}", status, error_text);
        }

        let rerank_response: ZhipuRerankResponse = response.json().await?;

        tracing::debug!(
            "Zhipu rerank returned {} results",
            rerank_response.results.len()
        );
        for result in &rerank_response.results {
            tracing::debug!(
                "  Result: index={}, relevance_score={:.6}",
                result.index,
                result.relevance_score
            );
        }

        Ok(rerank_response
            .results
            .into_iter()
            .map(|r| RerankItem {
                index: r.index,
                score: r.relevance_score,
            })
            .collect())
    }
}

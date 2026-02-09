//! 阿里云 DashScope Rerank

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::config::ResolvedService;
use crate::providers::common::{RerankItem, RerankProvider};

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

/// 阿里云 compatible-api 返回格式（results 在顶层）
#[derive(Debug, Deserialize)]
struct AliyunRerankResponse {
    results: Vec<AliyunRerankResult>,
}

#[derive(Debug, Deserialize)]
struct AliyunRerankResult {
    index: usize,
    relevance_score: f64,
}

#[derive(Debug, Deserialize)]
struct AliyunErrorResponse {
    code: Option<String>,
    message: Option<String>,
}

fn parse_aliyun_error(body: &str) -> String {
    if let Ok(err) = serde_json::from_str::<AliyunErrorResponse>(body) {
        if let (Some(code), Some(message)) = (err.code, err.message) {
            return format!("{}: {}", code, message);
        }
    }
    body.to_string()
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
            let msg = parse_aliyun_error(&error_text);
            tracing::error!("Aliyun rerank API error ({}): {}", status, msg);
            anyhow::bail!("Aliyun rerank API error ({}): {}", status, msg);
        }

        let rerank_response: AliyunRerankResponse = response.json().await?;

        let results = rerank_response.results;
        tracing::debug!("Aliyun rerank returned {} results", results.len());
        for result in &results {
            tracing::debug!(
                "  Result: index={}, relevance_score={:.6}",
                result.index,
                result.relevance_score
            );
        }

        Ok(results
            .into_iter()
            .map(|r| RerankItem {
                index: r.index,
                score: r.relevance_score,
            })
            .collect())
    }
}

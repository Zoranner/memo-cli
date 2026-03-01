//! 智谱 AI Embedding

use anyhow::Result;
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::time::Duration;

use crate::common::normalize_for_embedding;
use crate::config::ProviderConfig;
use crate::traits::EmbedProvider;

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
    pub fn new(config: &ProviderConfig, dimension: usize) -> Result<Self> {
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

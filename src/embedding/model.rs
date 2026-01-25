use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

/// Embedding 模型客户端 - 支持 OpenAI 兼容 API
pub struct EmbeddingModel {
    client: Client,
    api_key: String,
    model: String,
    base_url: String,
    #[allow(dead_code)]
    dimension: usize,
    provider: ProviderType,
}

/// 提供商类型
#[derive(Debug, Clone)]
enum ProviderType {
    OpenAI,
    Ollama,
}

impl EmbeddingModel {
    /// 创建新的 embedding 客户端
    ///
    /// # 参数
    /// - `api_key`: API 密钥
    /// - `model`: 模型名称
    /// - `base_url`: API 端点
    /// - `dimension`: embedding 维度(可选,自动推断)
    /// - `provider`: 提供商类型(可选: "openai", "ollama")
    pub fn new(
        api_key: String,
        model: String,
        base_url: Option<String>,
        dimension: Option<usize>,
        provider: Option<String>,
    ) -> Result<Self> {
        // 推断提供商和 base_url
        let (provider, base_url) = Self::infer_provider(&base_url, &provider);

        let client = Client::new();
        let dimension = dimension.unwrap_or_else(|| Self::infer_dimension(&model));

        Ok(Self {
            client,
            api_key,
            model,
            base_url,
            dimension,
            provider,
        })
    }

    /// 推断提供商类型
    fn infer_provider(base_url: &Option<String>, provider: &Option<String>) -> (ProviderType, String) {
        // 优先使用配置中指定的 provider
        if let Some(p) = provider {
            let provider_type = match p.to_lowercase().as_str() {
                "ollama" => ProviderType::Ollama,
                "openai" | _ => ProviderType::OpenAI,
            };
            
            let url = base_url.clone().unwrap_or_else(|| {
                match provider_type {
                    ProviderType::Ollama => "http://localhost:11434/api".to_string(),
                    ProviderType::OpenAI => "https://api.openai.com/v1".to_string(),
                }
            });
            
            return (provider_type, url);
        }

        // 根据 base_url 自动推断
        match base_url {
            Some(url) => {
                if url.contains("localhost") || url.contains("127.0.0.1") || url.contains("ollama") {
                    (ProviderType::Ollama, url.clone())
                } else {
                    (ProviderType::OpenAI, url.clone())
                }
            }
            None => {
                (ProviderType::OpenAI, "https://api.openai.com/v1".to_string())
            }
        }
    }

    /// 根据模型名称推断 embedding 维度
    fn infer_dimension(model: &str) -> usize {
        // 常见维度模式匹配
        if model.contains("-3-large") || model.contains("large") && model.contains("3072") {
            3072
        } else if model.contains("384") || model.contains("minilm") {
            384
        } else if model.contains("512") || model.contains("small") && model.contains("bge") {
            512
        } else if model.contains("768") || model.contains("nomic") || model.contains("v2") && model.contains("jina") {
            768
        } else if model.contains("1024") || model.contains("v3") || model.contains("v4") || model.contains("mxbai") {
            1024
        } else {
            // 默认维度
            1536
        }
    }

    /// 获取 embedding 维度
    pub fn dimension(&self) -> usize {
        self.dimension
    }

    /// 对单个文本生成 embedding
    pub async fn encode(&self, text: &str) -> Result<Vec<f32>> {
        match self.provider {
            ProviderType::Ollama => self.encode_ollama(text).await,
            ProviderType::OpenAI => self.encode_openai_compatible(text).await,
        }
    }

    /// OpenAI 兼容格式(OpenAI、Jina、Azure 等)
    async fn encode_openai_compatible(&self, text: &str) -> Result<Vec<f32>> {
        #[derive(Serialize)]
        struct Request {
            input: String,
            model: String,
        }

        #[derive(Deserialize)]
        struct Response {
            data: Vec<EmbeddingData>,
        }

        #[derive(Deserialize)]
        struct EmbeddingData {
            embedding: Vec<f32>,
        }

        let url = format!("{}/embeddings", self.base_url);
        let request = Request {
            input: text.to_string(),
            model: self.model.clone(),
        };

        let mut req = self.client.post(&url).json(&request);

        // 添加认证头
        if !self.api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {}", self.api_key));
        }

        let response = req
            .send()
            .await
            .context("Failed to send embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Embedding API error ({}): {}", status, error_text);
        }

        let api_response: Response = response
            .json()
            .await
            .context("Failed to parse embedding response")?;

        api_response
            .data
            .into_iter()
            .next()
            .map(|d| d.embedding)
            .context("No embedding returned")
    }

    /// Ollama 格式
    async fn encode_ollama(&self, text: &str) -> Result<Vec<f32>> {
        #[derive(Serialize)]
        struct Request {
            model: String,
            input: String,
        }

        #[derive(Deserialize)]
        struct Response {
            embeddings: Vec<Vec<f32>>,
        }

        let url = format!("{}/embed", self.base_url);
        let request = Request {
            model: self.model.clone(),
            input: text.to_string(),
        };

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send Ollama embedding request")?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("Ollama API error ({}): {}", status, error_text);
        }

        let api_response: Response = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        api_response
            .embeddings
            .into_iter()
            .next()
            .context("No embedding returned from Ollama")
    }

    /// 对多个文本批量生成 embeddings
    #[allow(dead_code)]
    pub async fn encode_batch(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>> {
        let mut results = Vec::with_capacity(texts.len());
        for text in texts {
            results.push(self.encode(&text).await?);
        }
        Ok(results)
    }
}

mod common;
mod config;
mod traits;

// 各供应商模块（feature gated）
#[cfg(feature = "aliyun")]
mod aliyun;
#[cfg(feature = "ollama")]
mod ollama;
#[cfg(feature = "openai")]
mod openai;
#[cfg(feature = "zhipu")]
mod zhipu;

pub use config::ProviderConfig;
pub use traits::{EmbedProvider, RerankItem, RerankProvider};

/// 创建 Embedding Provider
pub fn create_embed_provider(config: &ProviderConfig) -> anyhow::Result<Box<dyn EmbedProvider>> {
    let dimension = config
        .dimension
        .ok_or_else(|| anyhow::anyhow!("Missing 'dimension' for embed provider"))?;

    match config.provider_name.as_str() {
        #[cfg(feature = "aliyun")]
        "aliyun" => aliyun::embed::create(config, dimension),
        #[cfg(feature = "openai")]
        "openai" => openai::embed::create(config, dimension),
        #[cfg(feature = "ollama")]
        "ollama" => ollama::embed::create(config, dimension),
        #[cfg(feature = "zhipu")]
        "zhipu" => Ok(Box::new(zhipu::embed::ZhipuEmbedProvider::new(
            config, dimension,
        )?)),
        other => anyhow::bail!("Unknown or disabled embed provider: {}", other),
    }
}

/// 创建 Rerank Provider
pub fn create_rerank_provider(config: &ProviderConfig) -> anyhow::Result<Box<dyn RerankProvider>> {
    match config.provider_name.as_str() {
        #[cfg(feature = "aliyun")]
        "aliyun" => Ok(Box::new(aliyun::rerank::AliyunRerankProvider::new(config)?)),
        #[cfg(feature = "zhipu")]
        "zhipu" => Ok(Box::new(zhipu::rerank::ZhipuRerankProvider::new(config)?)),
        other => anyhow::bail!("Unknown or disabled rerank provider: {}", other),
    }
}

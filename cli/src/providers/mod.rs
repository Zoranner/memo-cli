mod aliyun;
mod common;
mod ollama;
mod openai;
mod zhipu;

pub use common::{EmbedProvider, RerankProvider};

use anyhow::Result;

use crate::config::ResolvedService;

/// 创建 Embedding Provider
pub fn create_embed_provider(config: &ResolvedService) -> Result<Box<dyn EmbedProvider>> {
    let dimension = config
        .get_int("dimension")
        .ok_or_else(|| anyhow::anyhow!("Missing 'dimension' in embed service config"))?
        as usize;

    match config.provider_name.as_str() {
        "aliyun" => aliyun::embed::create(config, dimension),
        "openai" => openai::embed::create(config, dimension),
        "ollama" => ollama::embed::create(config, dimension),
        "zhipu" => Ok(Box::new(zhipu::embed::ZhipuEmbedProvider::new(
            config, dimension,
        )?)),
        other => anyhow::bail!("Unknown embed provider: {}", other),
    }
}

/// 创建 Rerank Provider
pub fn create_rerank_provider(config: &ResolvedService) -> Result<Box<dyn RerankProvider>> {
    match config.provider_name.as_str() {
        "aliyun" => Ok(Box::new(aliyun::rerank::AliyunRerankProvider::new(config)?)),
        "zhipu" => Ok(Box::new(zhipu::rerank::ZhipuRerankProvider::new(config)?)),
        other => anyhow::bail!("Unknown rerank provider: {}", other),
    }
}

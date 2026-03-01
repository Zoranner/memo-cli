//! Ollama 本地 Embedding（OpenAI 兼容格式）

use anyhow::Result;

use crate::common::OpenaiCompatibleEmbed;
use crate::config::ProviderConfig;
use crate::traits::EmbedProvider;

pub fn create(config: &ProviderConfig, dimension: usize) -> Result<Box<dyn EmbedProvider>> {
    Ok(Box::new(OpenaiCompatibleEmbed::new(config, dimension)?))
}

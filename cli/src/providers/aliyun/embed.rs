//! 阿里云 DashScope Embedding（OpenAI 兼容格式）

use anyhow::Result;

use crate::config::ResolvedService;
use crate::providers::common::{EmbedProvider, OpenaiCompatibleEmbed};

pub fn create(config: &ResolvedService, dimension: usize) -> Result<Box<dyn EmbedProvider>> {
    Ok(Box::new(OpenaiCompatibleEmbed::new(config, dimension)?))
}

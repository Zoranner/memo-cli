use anyhow::Result;
use model_provider::{create_llm_provider, LlmProvider};

use crate::config::ResolvedService;

pub struct LlmClient {
    inner: Box<dyn LlmProvider>,
}

impl LlmClient {
    pub fn from_resolved(resolved: &ResolvedService) -> Result<Self> {
        let config = resolved.to_provider_config(None);
        Ok(Self {
            inner: create_llm_provider(&config)?,
        })
    }

    pub async fn chat(&self, prompt: &str) -> Result<String> {
        self.inner.chat(prompt).await
    }
}

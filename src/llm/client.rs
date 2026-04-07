use anyhow::Result;
use lmkit::{create_chat_provider, ChatProvider, ChatStream};

use crate::config::ResolvedService;

pub struct LlmClient {
    inner: Box<dyn ChatProvider>,
}

impl LlmClient {
    pub fn from_resolved(resolved: &ResolvedService) -> Result<Self> {
        let config = resolved.to_provider_config(None);
        Ok(Self {
            inner: create_chat_provider(&config)?,
        })
    }

    pub async fn chat(&self, prompt: &str) -> Result<String> {
        self.inner.chat(prompt).await.map_err(Into::into)
    }

    pub async fn chat_stream(&self, prompt: &str) -> Result<ChatStream> {
        self.inner.chat_stream(prompt).await.map_err(Into::into)
    }
}

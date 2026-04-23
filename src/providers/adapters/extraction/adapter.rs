use anyhow::{Context, Result};
use lmkit::{
    create_chat_provider, ChatMessage, ChatProvider as LmkitChatProvider, ChatRequest,
    ProviderConfig, RequestPreset, ResponseFormat,
};
use memo_engine::{ExtractionProvider, ExtractionResult};
use tokio::runtime::{Builder, Runtime};

use super::{
    parse_extraction_response_with_options, ExtractionCleanupOptions, EXTRACTION_SYSTEM_PROMPT,
};

pub(crate) struct LmkitExtractionAdapter {
    runtime: Runtime,
    provider: Box<dyn LmkitChatProvider>,
    options: ExtractionCleanupOptions,
}

impl LmkitExtractionAdapter {
    pub(crate) fn new_with_options(
        config: ProviderConfig,
        options: ExtractionCleanupOptions,
    ) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime for lmkit extraction")?;
        let provider =
            create_chat_provider(&config).context("failed to create lmkit chat provider")?;

        Ok(Self {
            runtime,
            provider,
            options,
        })
    }
}

impl ExtractionProvider for LmkitExtractionAdapter {
    fn extract(&self, text: &str) -> Result<ExtractionResult> {
        let request = ChatRequest {
            messages: vec![
                ChatMessage::system(EXTRACTION_SYSTEM_PROMPT),
                ChatMessage::user(text),
            ],
            response_format: Some(ResponseFormat::JsonObject),
            preset: Some(RequestPreset::Execution),
            temperature: Some(0.0),
            ..Default::default()
        };
        let response = self
            .runtime
            .block_on(self.provider.complete(&request))
            .context("lmkit extraction request failed")?;
        let content = response
            .content
            .as_deref()
            .with_context(|| "lmkit extraction response missing content".to_string())?;

        parse_extraction_response_with_options(content, self.options)
    }
}

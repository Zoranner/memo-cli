use anyhow::{Context, Result};
use lmkit::{create_embed_provider, EmbedProvider as LmkitEmbedProvider, ProviderConfig};
use memo_engine::EmbeddingProvider;
use tokio::runtime::{Builder, Runtime};

pub(crate) struct LmkitEmbeddingAdapter {
    runtime: Runtime,
    provider: Box<dyn LmkitEmbedProvider>,
    dimension: usize,
}

impl LmkitEmbeddingAdapter {
    pub(crate) fn new(config: ProviderConfig) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime for lmkit embedding")?;
        let provider =
            create_embed_provider(&config).context("failed to create lmkit embed provider")?;
        let dimension = provider.dimension();

        Ok(Self {
            runtime,
            provider,
            dimension,
        })
    }
}

impl EmbeddingProvider for LmkitEmbeddingAdapter {
    fn dimension(&self) -> usize {
        self.dimension
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        self.runtime
            .block_on(self.provider.encode(text))
            .context("lmkit embed request failed")
    }
}

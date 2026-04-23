use anyhow::{Context, Result};
use lmkit::{create_rerank_provider, ProviderConfig};
use memo_engine::{RerankProvider, RerankScore};
use tokio::runtime::{Builder, Runtime};

pub(crate) struct LmkitRerankAdapter {
    runtime: Runtime,
    provider: Box<dyn lmkit::RerankProvider>,
}

impl LmkitRerankAdapter {
    pub(crate) fn new(config: ProviderConfig) -> Result<Self> {
        let runtime = Builder::new_multi_thread()
            .enable_all()
            .build()
            .context("failed to create tokio runtime for lmkit rerank")?;
        let provider =
            create_rerank_provider(&config).context("failed to create lmkit rerank provider")?;

        Ok(Self { runtime, provider })
    }
}

impl RerankProvider for LmkitRerankAdapter {
    fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<RerankScore>> {
        let refs = documents.iter().map(String::as_str).collect::<Vec<_>>();
        let items = self
            .runtime
            .block_on(self.provider.rerank(query, &refs, Some(refs.len())))
            .context("lmkit rerank request failed")?;

        Ok(items
            .into_iter()
            .map(|item| RerankScore {
                index: item.index,
                score: item.score as f32,
            })
            .collect())
    }
}

use std::{thread, time::Duration};

use anyhow::Result;
use memo_engine::{
    EmbeddingProvider, ExtractionProvider, ExtractionResult, RerankProvider, RerankScore,
};
use tracing::warn;

use crate::provider_status::ProviderRuntimeRecorder;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct ProviderRetryPolicy {
    pub max_retries: usize,
    pub retry_backoff_ms: u64,
}

impl ProviderRetryPolicy {
    pub(crate) fn new(max_retries: Option<usize>, retry_backoff_ms: Option<u64>) -> Self {
        Self {
            max_retries: max_retries.unwrap_or(0),
            retry_backoff_ms: retry_backoff_ms.unwrap_or(0),
        }
    }
}

pub(crate) struct RetryingEmbeddingProvider<P> {
    inner: P,
    provider_ref: String,
    policy: ProviderRetryPolicy,
    recorder: ProviderRuntimeRecorder,
}

impl<P> RetryingEmbeddingProvider<P> {
    pub(crate) fn new(
        inner: P,
        provider_ref: impl Into<String>,
        policy: ProviderRetryPolicy,
        recorder: ProviderRuntimeRecorder,
    ) -> Self {
        Self {
            inner,
            provider_ref: provider_ref.into(),
            policy,
            recorder,
        }
    }
}

impl<P> EmbeddingProvider for RetryingEmbeddingProvider<P>
where
    P: EmbeddingProvider,
{
    fn dimension(&self) -> usize {
        self.inner.dimension()
    }

    fn embed_text(&self, text: &str) -> Result<Vec<f32>> {
        retry_with_policy(
            "embedding",
            &self.provider_ref,
            self.policy,
            &self.recorder,
            || self.inner.embed_text(text),
        )
    }
}

pub(crate) struct RetryingExtractionProvider<P> {
    inner: P,
    provider_ref: String,
    policy: ProviderRetryPolicy,
    recorder: ProviderRuntimeRecorder,
}

impl<P> RetryingExtractionProvider<P> {
    pub(crate) fn new(
        inner: P,
        provider_ref: impl Into<String>,
        policy: ProviderRetryPolicy,
        recorder: ProviderRuntimeRecorder,
    ) -> Self {
        Self {
            inner,
            provider_ref: provider_ref.into(),
            policy,
            recorder,
        }
    }
}

impl<P> ExtractionProvider for RetryingExtractionProvider<P>
where
    P: ExtractionProvider,
{
    fn extract(&self, text: &str) -> Result<ExtractionResult> {
        retry_with_policy(
            "extraction",
            &self.provider_ref,
            self.policy,
            &self.recorder,
            || self.inner.extract(text),
        )
    }
}

pub(crate) struct RetryingRerankProvider<P> {
    inner: P,
    provider_ref: String,
    policy: ProviderRetryPolicy,
    recorder: ProviderRuntimeRecorder,
}

impl<P> RetryingRerankProvider<P> {
    pub(crate) fn new(
        inner: P,
        provider_ref: impl Into<String>,
        policy: ProviderRetryPolicy,
        recorder: ProviderRuntimeRecorder,
    ) -> Self {
        Self {
            inner,
            provider_ref: provider_ref.into(),
            policy,
            recorder,
        }
    }
}

impl<P> RerankProvider for RetryingRerankProvider<P>
where
    P: RerankProvider,
{
    fn rerank(&self, query: &str, documents: &[String]) -> Result<Vec<RerankScore>> {
        retry_with_policy(
            "rerank",
            &self.provider_ref,
            self.policy,
            &self.recorder,
            || self.inner.rerank(query, documents),
        )
    }
}

fn retry_with_policy<T>(
    capability: &'static str,
    provider_ref: &str,
    policy: ProviderRetryPolicy,
    recorder: &ProviderRuntimeRecorder,
    mut action: impl FnMut() -> Result<T>,
) -> Result<T> {
    let max_attempts = policy.max_retries.saturating_add(1);
    for attempt in 0..max_attempts {
        match action() {
            Ok(value) => {
                recorder.record_success(capability, provider_ref)?;
                return Ok(value);
            }
            Err(error) => {
                let retryable = is_retryable_provider_error(&error);
                if attempt + 1 >= max_attempts || !retryable {
                    recorder.record_failure(capability, provider_ref, &error)?;
                    return Err(error);
                }

                warn!(
                    capability,
                    provider_ref,
                    attempt = attempt + 1,
                    max_attempts,
                    error = %error,
                    "provider call failed; retrying"
                );
                if policy.retry_backoff_ms > 0 {
                    let backoff_ms = policy.retry_backoff_ms.saturating_mul((attempt + 1) as u64);
                    thread::sleep(Duration::from_millis(backoff_ms));
                }
            }
        }
    }

    unreachable!("retry loop always returns or errors")
}

fn is_retryable_provider_error(error: &anyhow::Error) -> bool {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<lmkit::Error>())
        .is_some_and(lmkit::Error::is_retryable)
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use anyhow::Result;
    use memo_engine::{
        EmbeddingProvider, ExtractionProvider, ExtractionResult, RerankProvider, RerankScore,
    };
    use tempfile::TempDir;

    use super::{
        is_retryable_provider_error, ProviderRetryPolicy, RetryingEmbeddingProvider,
        RetryingExtractionProvider, RetryingRerankProvider,
    };
    use crate::provider_status::{
        load_provider_runtime_summary, ProviderHealth, ProviderRuntimeRecorder,
    };

    #[derive(Clone)]
    struct FlakyEmbeddingProvider {
        calls: Arc<AtomicUsize>,
        succeed_on: usize,
    }

    impl EmbeddingProvider for FlakyEmbeddingProvider {
        fn dimension(&self) -> usize {
            4
        }

        fn embed_text(&self, _text: &str) -> Result<Vec<f32>> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call < self.succeed_on {
                return Err(anyhow::Error::new(lmkit::Error::Api {
                    status: 429,
                    message: "retry later".to_string(),
                }));
            }
            Ok(vec![1.0, 0.0, 0.0, 0.0])
        }
    }

    #[derive(Clone)]
    struct FlakyExtractionProvider {
        calls: Arc<AtomicUsize>,
    }

    impl ExtractionProvider for FlakyExtractionProvider {
        fn extract(&self, _text: &str) -> Result<ExtractionResult> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == 1 {
                return Err(anyhow::Error::new(lmkit::Error::Api {
                    status: 400,
                    message: "bad request".to_string(),
                }));
            }
            Ok(ExtractionResult::default())
        }
    }

    #[derive(Clone)]
    struct FlakyRerankProvider {
        calls: Arc<AtomicUsize>,
    }

    impl RerankProvider for FlakyRerankProvider {
        fn rerank(&self, _query: &str, _documents: &[String]) -> Result<Vec<RerankScore>> {
            let call = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            if call == 1 {
                return Err(anyhow::Error::new(lmkit::Error::Api {
                    status: 503,
                    message: "upstream unavailable".to_string(),
                }));
            }
            Ok(vec![RerankScore {
                index: 0,
                score: 1.0,
            }])
        }
    }

    #[test]
    fn retrying_embedding_provider_retries_retryable_errors() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let temp = TempDir::new()?;
        let provider = RetryingEmbeddingProvider::new(
            FlakyEmbeddingProvider {
                calls: Arc::clone(&calls),
                succeed_on: 3,
            },
            "openai.embed",
            ProviderRetryPolicy::new(Some(2), Some(0)),
            ProviderRuntimeRecorder::new(temp.path()),
        );

        let vector = provider.embed_text("hello")?;

        assert_eq!(vector.len(), 4);
        assert_eq!(calls.load(Ordering::SeqCst), 3);
        let summary = load_provider_runtime_summary(temp.path());
        let status = summary
            .statuses
            .iter()
            .find(|status| status.capability == "embedding")
            .expect("expected embedding status");
        assert_eq!(status.status, ProviderHealth::Ok);
        Ok(())
    }

    #[test]
    fn retrying_extraction_provider_stops_after_retry_budget() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let temp = TempDir::new()?;
        let provider = RetryingExtractionProvider::new(
            FlakyExtractionProvider {
                calls: Arc::clone(&calls),
            },
            "openai.extract",
            ProviderRetryPolicy::new(Some(0), Some(0)),
            ProviderRuntimeRecorder::new(temp.path()),
        );

        let error = provider
            .extract("hello")
            .expect_err("expected provider error");
        assert!(!is_retryable_provider_error(&error));
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let summary = load_provider_runtime_summary(temp.path());
        let status = summary
            .statuses
            .iter()
            .find(|status| status.capability == "extraction")
            .expect("expected extraction status");
        assert_eq!(status.status, ProviderHealth::Degraded);
        Ok(())
    }

    #[test]
    fn retrying_rerank_provider_retries_service_errors() -> Result<()> {
        let calls = Arc::new(AtomicUsize::new(0));
        let temp = TempDir::new()?;
        let provider = RetryingRerankProvider::new(
            FlakyRerankProvider {
                calls: Arc::clone(&calls),
            },
            "aliyun.rerank",
            ProviderRetryPolicy::new(Some(1), Some(0)),
            ProviderRuntimeRecorder::new(temp.path()),
        );

        let ranked = provider.rerank("hello", &["a".to_string()])?;

        assert_eq!(ranked.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 2);
        let summary = load_provider_runtime_summary(temp.path());
        let status = summary
            .statuses
            .iter()
            .find(|status| status.capability == "rerank")
            .expect("expected rerank status");
        assert_eq!(status.status, ProviderHealth::Ok);
        Ok(())
    }
}

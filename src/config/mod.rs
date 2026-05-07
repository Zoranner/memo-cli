mod app_home;
mod file_config;
mod provider_config;
mod templates;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use memo_engine::EngineConfig;

use crate::providers::adapters::embedding::LmkitEmbeddingAdapter;
use crate::providers::adapters::extraction::{ExtractionCleanupOptions, LmkitExtractionAdapter};
use crate::providers::adapters::rerank::LmkitRerankAdapter;
use crate::providers::runtime::{
    ProviderRetryPolicy, RetryingEmbeddingProvider, RetryingExtractionProvider,
    RetryingRerankProvider,
};
use crate::providers::status::ProviderRuntimeRecorder;
use crate::providers::status::{
    ProviderCapabilityReadiness, ProviderReadiness, ProviderReadinessSummary,
    ProviderRuntimeSummary,
};

pub(crate) use app_home::{initialize_app_home, InitReport};
use file_config::{load_file_config, resolve_relative_to_dir, ExtractConfig};
use provider_config::{load_provider_config, provider_ref_uses_placeholder_key};

pub(crate) fn build_engine_config(
    data_dir: impl Into<PathBuf>,
    config_dir: &Path,
) -> Result<EngineConfig> {
    let data_dir = data_dir.into();
    let mut engine_config = EngineConfig::new(&data_dir);
    let provider_runtime = ProviderRuntimeRecorder::new(&data_dir);
    let Some(file_config) = load_file_config(config_dir)? else {
        return Ok(engine_config);
    };

    if let Some(limit) = file_config.engine.l3_cache_limit {
        engine_config.l3_cache_limit = limit;
    }

    if let Some(provider_ref) = file_config.embed.embedding_provider.as_deref() {
        if provider_ref_uses_placeholder_key(config_dir, provider_ref)
            .with_context(|| format!("failed to resolve embedding provider `{provider_ref}`"))?
        {
        } else {
            let provider_config = load_provider_config(config_dir, provider_ref, "embedding")?;
            let adapter = RetryingEmbeddingProvider::new(
                LmkitEmbeddingAdapter::new(provider_config)?,
                provider_ref,
                ProviderRetryPolicy::new(
                    file_config.embed.max_retries,
                    file_config.embed.retry_backoff_ms,
                ),
                provider_runtime.clone(),
            );
            engine_config = engine_config.with_embedding_provider(Arc::new(adapter));
        }
    }

    if let Some(provider_ref) = file_config.extract.extraction_provider.as_deref() {
        if provider_ref_uses_placeholder_key(config_dir, provider_ref)
            .with_context(|| format!("failed to resolve extraction provider `{provider_ref}`"))?
        {
        } else {
            let provider_config = load_provider_config(config_dir, provider_ref, "extraction")?;
            let adapter = RetryingExtractionProvider::new(
                LmkitExtractionAdapter::new_with_options(
                    provider_config,
                    extraction_cleanup_options(&file_config.extract),
                )?,
                provider_ref,
                ProviderRetryPolicy::new(
                    file_config.extract.max_retries,
                    file_config.extract.retry_backoff_ms,
                ),
                provider_runtime.clone(),
            );
            engine_config = engine_config.with_extraction_provider(Arc::new(adapter));
        }
    }

    if let Some(provider_ref) = file_config.rerank.rerank_provider.as_deref() {
        if provider_ref_uses_placeholder_key(config_dir, provider_ref)
            .with_context(|| format!("failed to resolve rerank provider `{provider_ref}`"))?
        {
        } else {
            let provider_config = load_provider_config(config_dir, provider_ref, "rerank")?;
            let adapter = RetryingRerankProvider::new(
                LmkitRerankAdapter::new(provider_config)?,
                provider_ref,
                ProviderRetryPolicy::new(
                    file_config.rerank.max_retries,
                    file_config.rerank.retry_backoff_ms,
                ),
                provider_runtime,
            );
            engine_config = engine_config.with_rerank_provider(Arc::new(adapter));
        }
    }

    Ok(engine_config)
}

pub(crate) fn resolve_configured_data_dir(config_dir: &Path) -> Result<Option<PathBuf>> {
    let Some(file_config) = load_file_config(config_dir)? else {
        return Ok(None);
    };
    Ok(file_config
        .storage
        .data_dir
        .as_deref()
        .map(|value| resolve_relative_to_dir(config_dir, Path::new(value))))
}

pub(crate) fn load_provider_readiness(
    config_dir: &Path,
    runtime: &ProviderRuntimeSummary,
) -> ProviderReadinessSummary {
    let file_config = match load_file_config(config_dir) {
        Ok(value) => value,
        Err(error) => {
            return ProviderReadinessSummary {
                capabilities: vec![ProviderCapabilityReadiness {
                    capability: "config".to_string(),
                    provider_ref: None,
                    status: ProviderReadiness::Degraded,
                    detail: Some(error.to_string()),
                }],
            };
        }
    };

    let mut capabilities = Vec::new();
    let Some(file_config) = file_config else {
        for capability in ["embedding", "extraction", "rerank"] {
            capabilities.push(ProviderCapabilityReadiness {
                capability: capability.to_string(),
                provider_ref: None,
                status: ProviderReadiness::NotConfigured,
                detail: None,
            });
        }
        return ProviderReadinessSummary { capabilities };
    };

    capabilities.push(provider_readiness_for_ref(
        config_dir,
        runtime,
        "embedding",
        file_config.embed.embedding_provider.as_deref(),
    ));
    capabilities.push(provider_readiness_for_ref(
        config_dir,
        runtime,
        "extraction",
        file_config.extract.extraction_provider.as_deref(),
    ));
    capabilities.push(provider_readiness_for_ref(
        config_dir,
        runtime,
        "rerank",
        file_config.rerank.rerank_provider.as_deref(),
    ));

    ProviderReadinessSummary { capabilities }
}

fn provider_readiness_for_ref(
    config_dir: &Path,
    runtime: &ProviderRuntimeSummary,
    capability: &str,
    provider_ref: Option<&str>,
) -> ProviderCapabilityReadiness {
    let Some(provider_ref) = provider_ref else {
        return ProviderCapabilityReadiness {
            capability: capability.to_string(),
            provider_ref: None,
            status: ProviderReadiness::NotConfigured,
            detail: None,
        };
    };

    match provider_ref_uses_placeholder_key(config_dir, provider_ref) {
        Ok(true) => {
            return ProviderCapabilityReadiness {
                capability: capability.to_string(),
                provider_ref: Some(provider_ref.to_string()),
                status: ProviderReadiness::PlaceholderKey,
                detail: Some("provider api_key is still a template placeholder".to_string()),
            };
        }
        Err(error) => {
            return ProviderCapabilityReadiness {
                capability: capability.to_string(),
                provider_ref: Some(provider_ref.to_string()),
                status: ProviderReadiness::Degraded,
                detail: Some(error.to_string()),
            };
        }
        Ok(false) => {}
    }

    if let Some(status) = runtime
        .statuses
        .iter()
        .find(|status| status.capability == capability)
    {
        return ProviderCapabilityReadiness {
            capability: capability.to_string(),
            provider_ref: Some(status.provider_ref.clone()),
            status: match status.status {
                crate::providers::status::ProviderHealth::Ok => ProviderReadiness::Ok,
                crate::providers::status::ProviderHealth::Degraded => ProviderReadiness::Degraded,
            },
            detail: status.last_error.clone(),
        };
    }

    ProviderCapabilityReadiness {
        capability: capability.to_string(),
        provider_ref: Some(provider_ref.to_string()),
        status: ProviderReadiness::Configured,
        detail: None,
    }
}

fn extraction_cleanup_options(config: &ExtractConfig) -> ExtractionCleanupOptions {
    ExtractionCleanupOptions {
        min_confidence: config.min_confidence.unwrap_or(0.5),
        normalize_predicates: config.normalize_predicates.unwrap_or(true),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use tempfile::TempDir;

    use crate::providers::status::{ProviderReadiness, ProviderRuntimeSummary};

    use super::{
        build_engine_config,
        file_config::parse_app_config,
        initialize_app_home, load_provider_readiness,
        provider_config::{parse_providers_config, provider_ref_uses_placeholder_key_from_text},
        resolve_configured_data_dir,
    };

    #[test]
    fn init_writes_current_templates_into_fixed_config_dir() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        let data_dir = temp.path().join("data");

        let report = initialize_app_home(&config_dir, &data_dir)?;

        assert!(report.config_created);
        assert!(report.providers_created);
        assert!(config_dir.join("config.toml").exists());
        assert!(config_dir.join("providers.toml").exists());
        Ok(())
    }

    #[test]
    fn init_bootstraps_sqlite_and_index_dirs_in_target_data_dir() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        let data_dir = temp.path().join("memory-data");

        initialize_app_home(&config_dir, &data_dir)?;

        assert!(data_dir.join("memory.db").exists());
        assert!(data_dir.join("text-index").is_dir());
        Ok(())
    }

    #[test]
    fn build_engine_config_loads_embedding_provider_from_fixed_config_root() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        let data_dir = temp.path().join("memory-data");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[embed]\nembedding_provider = \"openai.embed\"\n[extract]\nextraction_provider = \"openai.extract\"\n",
        )?;
        fs::write(
            config_dir.join("providers.toml"),
            "[openai]\napi_key = \"sk-test\"\n[openai.embed]\nbase_url = \"https://api.openai.com/v1\"\nmodel = \"text-embedding-3-small\"\ndimension = 1536\n[openai.extract]\nbase_url = \"https://api.openai.com/v1\"\nmodel = \"gpt-4o-mini\"\n",
        )?;

        let config = build_engine_config(&data_dir, &config_dir)?;

        assert_eq!(config.vector_dimension, 1536);
        let provider = config
            .embedding_provider
            .as_ref()
            .expect("expected embedding provider to be loaded");
        assert_eq!(provider.dimension(), 1536);
        assert!(config.extraction_provider.is_some());
        Ok(())
    }

    #[test]
    fn placeholder_provider_key_is_reported_and_not_loaded() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        let data_dir = temp.path().join("memory-data");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[embed]\nembedding_provider = \"openai.embed\"\n[extract]\nextraction_provider = \"openai.extract\"\n",
        )?;
        fs::write(
            config_dir.join("providers.toml"),
            "[openai]\napi_key = \"sk-your-openai-api-key\"\n[openai.embed]\nbase_url = \"https://api.openai.com/v1\"\nmodel = \"text-embedding-3-small\"\ndimension = 1536\n[openai.extract]\nbase_url = \"https://api.openai.com/v1\"\nmodel = \"gpt-4o-mini\"\n",
        )?;

        let config = build_engine_config(&data_dir, &config_dir)?;
        assert!(config.embedding_provider.is_none());
        assert!(config.extraction_provider.is_none());

        let readiness = load_provider_readiness(&config_dir, &ProviderRuntimeSummary::default());
        assert!(readiness.capabilities.iter().any(|capability| {
            capability.capability == "embedding"
                && capability.status == ProviderReadiness::PlaceholderKey
        }));
        assert!(readiness.capabilities.iter().any(|capability| {
            capability.capability == "extraction"
                && capability.status == ProviderReadiness::PlaceholderKey
        }));
        Ok(())
    }

    #[test]
    fn provider_placeholder_detection_allows_local_ollama_empty_key() -> Result<()> {
        let providers = "[ollama]\napi_key = \"\"\n[ollama.embed]\nbase_url = \"http://127.0.0.1:11434/v1\"\nmodel = \"bge-m3\"\ndimension = 1024\n";

        assert!(!provider_ref_uses_placeholder_key_from_text(
            providers,
            "ollama.embed"
        )?);
        Ok(())
    }

    #[test]
    fn build_engine_config_loads_rerank_provider_from_fixed_config_root() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        let data_dir = temp.path().join("memory-data");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[rerank]\nrerank_provider = \"aliyun.rerank\"\n",
        )?;
        fs::write(
            config_dir.join("providers.toml"),
            "[aliyun]\napi_key = \"sk-test\"\n[aliyun.rerank]\nbase_url = \"https://dashscope.aliyuncs.com/api/v1\"\nmodel = \"gte-rerank\"\n",
        )?;

        let config = build_engine_config(&data_dir, &config_dir)?;

        assert!(config.rerank_provider.is_some());
        Ok(())
    }

    #[test]
    fn build_engine_config_rejects_invalid_provider_ref() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        let data_dir = temp.path().join("memory-data");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[embed]\nembedding_provider = \"openai\"\n",
        )?;
        fs::write(
            config_dir.join("providers.toml"),
            "[openai]\napi_key = \"sk-test\"\n",
        )?;

        let error = match build_engine_config(&data_dir, &config_dir) {
            Ok(_) => panic!("expected invalid provider ref"),
            Err(error) => error,
        };
        assert!(error
            .to_string()
            .contains("failed to resolve embedding provider `openai`"));
        assert!(error.chain().any(|cause| cause
            .to_string()
            .contains("must look like `<provider>.<service>`")));
        Ok(())
    }

    #[test]
    fn parse_app_config_reads_provider_retry_settings() -> Result<()> {
        let config = parse_app_config(
            "[storage]\ndata_dir = \"memory-data\"\n\
             [embed]\nembedding_provider = \"openai.embed\"\nmax_retries = 2\nretry_backoff_ms = 150\n\
             [extract]\nextraction_provider = \"openai.extract\"\nmax_retries = 3\nretry_backoff_ms = 250\n\
             [rerank]\nrerank_provider = \"aliyun.rerank\"\nmax_retries = 1\nretry_backoff_ms = 50\n",
        )?;

        assert_eq!(config.storage.data_dir.as_deref(), Some("memory-data"));
        assert_eq!(config.embed.max_retries, Some(2));
        assert_eq!(config.embed.retry_backoff_ms, Some(150));
        assert_eq!(config.extract.max_retries, Some(3));
        assert_eq!(config.extract.retry_backoff_ms, Some(250));
        assert_eq!(config.rerank.max_retries, Some(1));
        assert_eq!(config.rerank.retry_backoff_ms, Some(50));
        Ok(())
    }

    #[test]
    fn build_engine_config_reads_l3_cache_limit_from_app_config() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        let data_dir = temp.path().join("memory-data");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[engine]\nl3_cache_limit = 7\n",
        )?;

        let config = build_engine_config(&data_dir, &config_dir)?;

        assert_eq!(config.l3_cache_limit, 7);
        Ok(())
    }

    #[test]
    fn parse_providers_config_reads_timeout_and_concurrency_hints() -> Result<()> {
        let providers = parse_providers_config(
            "[openai]\napi_key = \"sk-test\"\n\
             [openai.embed]\nbase_url = \"https://api.openai.com/v1\"\nmodel = \"text-embedding-3-small\"\ndimension = 1536\ntimeout_ms = 1200\nmax_concurrent = 4\n",
        )?;

        let embed = providers
            .get("openai")
            .and_then(|provider| provider.services.get("embed"))
            .expect("expected openai.embed service");
        assert_eq!(embed.timeout_ms, Some(1200));
        assert_eq!(embed.max_concurrent, Some(4));
        Ok(())
    }

    #[test]
    fn resolve_configured_data_dir_resolves_relative_path_against_fixed_config_dir() -> Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[storage]\ndata_dir = \"memory-data\"\n",
        )?;

        let data_dir =
            resolve_configured_data_dir(&config_dir)?.expect("expected configured data dir");

        assert_eq!(data_dir, config_dir.join("memory-data"));
        Ok(())
    }
}

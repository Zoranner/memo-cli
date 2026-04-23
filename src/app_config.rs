use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use anyhow::{Context, Result};
use lmkit::{Provider, ProviderConfig};
use memo_engine::{EngineConfig, MemoryEngine};

use crate::lmkit_adapter::LmkitEmbeddingAdapter;
use crate::lmkit_extraction_adapter::{ExtractionCleanupOptions, LmkitExtractionAdapter};
use crate::lmkit_rerank_adapter::LmkitRerankAdapter;
use crate::provider_runtime::{
    ProviderRetryPolicy, RetryingEmbeddingProvider, RetryingExtractionProvider,
    RetryingRerankProvider,
};
use crate::provider_status::ProviderRuntimeRecorder;

const CONFIG_TEMPLATE: &str = include_str!("templates/config.toml");
const PROVIDERS_TEMPLATE: &str = include_str!("templates/providers.toml");

#[derive(Debug, Default)]
struct EmbedConfig {
    embedding_provider: Option<String>,
    duplicate_threshold: Option<f32>,
    max_retries: Option<usize>,
    retry_backoff_ms: Option<u64>,
}

#[derive(Debug, Default)]
struct ExtractConfig {
    extraction_provider: Option<String>,
    min_confidence: Option<f32>,
    normalize_predicates: Option<bool>,
    max_retries: Option<usize>,
    retry_backoff_ms: Option<u64>,
}

#[derive(Debug, Default)]
struct RerankConfig {
    rerank_provider: Option<String>,
    max_retries: Option<usize>,
    retry_backoff_ms: Option<u64>,
}

#[derive(Debug, Default)]
struct StorageConfig {
    data_dir: Option<String>,
}

#[derive(Debug, Default)]
struct FileConfig {
    storage: StorageConfig,
    embed: EmbedConfig,
    extract: ExtractConfig,
    rerank: RerankConfig,
}

#[derive(Debug, Default)]
struct ProviderEntry {
    api_key: String,
    services: HashMap<String, ProviderService>,
}

#[derive(Debug, Default)]
struct ProviderService {
    base_url: Option<String>,
    model: Option<String>,
    dimension: Option<usize>,
    timeout_ms: Option<u64>,
    max_concurrent: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitReport {
    pub config_created: bool,
    pub providers_created: bool,
}

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

    let _duplicate_threshold = file_config.embed.duplicate_threshold;

    if let Some(provider_ref) = file_config.embed.embedding_provider.as_deref() {
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

    if let Some(provider_ref) = file_config.extract.extraction_provider.as_deref() {
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

    if let Some(provider_ref) = file_config.rerank.rerank_provider.as_deref() {
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

fn extraction_cleanup_options(config: &ExtractConfig) -> ExtractionCleanupOptions {
    ExtractionCleanupOptions {
        min_confidence: config.min_confidence.unwrap_or(0.5),
        normalize_predicates: config.normalize_predicates.unwrap_or(true),
    }
}

pub(crate) fn initialize_app_home(config_dir: &Path, data_dir: &Path) -> Result<InitReport> {
    fs::create_dir_all(config_dir)
        .with_context(|| format!("failed to create config dir: {}", config_dir.display()))?;
    fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create data dir: {}", data_dir.display()))?;

    let config_created = write_if_missing(&config_dir.join("config.toml"), CONFIG_TEMPLATE)?;
    let providers_created =
        write_if_missing(&config_dir.join("providers.toml"), PROVIDERS_TEMPLATE)?;
    MemoryEngine::open(EngineConfig::new(data_dir))?;

    Ok(InitReport {
        config_created,
        providers_created,
    })
}

fn write_if_missing(path: &Path, contents: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }

    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

fn load_file_config(config_dir: &Path) -> Result<Option<FileConfig>> {
    let config_path = config_dir.join("config.toml");
    if !config_path.exists() {
        return Ok(None);
    }

    let config_text = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config file: {}", config_path.display()))?;
    let file_config = parse_app_config(&config_text)
        .with_context(|| format!("failed to parse config file: {}", config_path.display()))?;
    Ok(Some(file_config))
}

fn load_provider_config(
    config_dir: &Path,
    provider_ref: &str,
    capability: &str,
) -> Result<ProviderConfig> {
    let providers_path = config_dir.join("providers.toml");
    let providers_text = fs::read_to_string(&providers_path).with_context(|| {
        format!(
            "failed to read providers file: {}",
            providers_path.display()
        )
    })?;

    resolve_provider_config(&providers_text, provider_ref)
        .with_context(|| format!("failed to resolve {capability} provider `{provider_ref}`"))
}

fn resolve_provider_config(providers_toml: &str, provider_ref: &str) -> Result<ProviderConfig> {
    let providers =
        parse_providers_config(providers_toml).context("failed to parse providers.toml")?;
    let (provider_name, service_name) = split_provider_ref(provider_ref)?;
    let provider_entry = providers
        .get(provider_name)
        .with_context(|| format!("provider `{provider_name}` not found"))?;
    let service_entry = provider_entry
        .services
        .get(service_name)
        .with_context(|| format!("service `{service_name}` not found under `{provider_name}`"))?;

    let provider: Provider = provider_name.parse()?;
    let base_url = service_entry
        .base_url
        .clone()
        .with_context(|| "missing `base_url`".to_string())?;
    let model = service_entry
        .model
        .clone()
        .with_context(|| "missing `model`".to_string())?;

    let mut config = ProviderConfig::new(provider, &provider_entry.api_key, base_url, model);
    config.dimension = service_entry.dimension;
    config.timeout = service_entry.timeout_ms.map(Duration::from_millis);
    config.max_concurrent = service_entry.max_concurrent;

    Ok(config)
}

fn split_provider_ref(provider_ref: &str) -> Result<(&str, &str)> {
    let (provider_name, service_name) = provider_ref.split_once('.').with_context(|| {
        format!("provider ref `{provider_ref}` must look like `<provider>.<service>`")
    })?;

    if provider_name.is_empty() || service_name.is_empty() {
        anyhow::bail!("provider ref `{provider_ref}` must look like `<provider>.<service>`");
    }

    Ok((provider_name, service_name))
}

fn resolve_relative_to_dir(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn parse_app_config(contents: &str) -> Result<FileConfig> {
    let mut config = FileConfig::default();
    let mut section: Option<String> = None;

    for (line_no, raw_line) in contents.lines().enumerate() {
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(section_name) = parse_section_header(line) {
            section = Some(section_name.to_string());
            continue;
        }

        let (key, value) = parse_key_value(line)
            .with_context(|| format!("invalid config line {}", line_no + 1))?;
        match section.as_deref() {
            Some("storage") => {
                if key == "data_dir" {
                    config.storage.data_dir = Some(parse_string(value)?.to_string());
                }
            }
            Some("embed") => match key {
                "embedding_provider" => {
                    config.embed.embedding_provider = Some(parse_string(value)?.to_string());
                }
                "duplicate_threshold" => {
                    config.embed.duplicate_threshold = Some(value.parse::<f32>()?);
                }
                "max_retries" => {
                    config.embed.max_retries = Some(value.parse::<usize>()?);
                }
                "retry_backoff_ms" => {
                    config.embed.retry_backoff_ms = Some(value.parse::<u64>()?);
                }
                _ => {}
            },
            Some("extract") => match key {
                "extraction_provider" => {
                    config.extract.extraction_provider = Some(parse_string(value)?.to_string());
                }
                "min_confidence" => {
                    config.extract.min_confidence = Some(value.parse::<f32>()?);
                }
                "normalize_predicates" => {
                    config.extract.normalize_predicates = Some(parse_bool(value)?);
                }
                "max_retries" => {
                    config.extract.max_retries = Some(value.parse::<usize>()?);
                }
                "retry_backoff_ms" => {
                    config.extract.retry_backoff_ms = Some(value.parse::<u64>()?);
                }
                _ => {}
            },
            Some("rerank") => match key {
                "rerank_provider" => {
                    config.rerank.rerank_provider = Some(parse_string(value)?.to_string());
                }
                "max_retries" => {
                    config.rerank.max_retries = Some(value.parse::<usize>()?);
                }
                "retry_backoff_ms" => {
                    config.rerank.retry_backoff_ms = Some(value.parse::<u64>()?);
                }
                _ => {}
            },
            _ => {}
        }
    }

    Ok(config)
}

fn parse_providers_config(contents: &str) -> Result<HashMap<String, ProviderEntry>> {
    enum Section {
        Provider(String),
        Service { provider: String, service: String },
    }

    let mut providers = HashMap::<String, ProviderEntry>::new();
    let mut section: Option<Section> = None;

    for (line_no, raw_line) in contents.lines().enumerate() {
        let line = strip_comments(raw_line).trim();
        if line.is_empty() {
            continue;
        }

        if let Some(section_name) = parse_section_header(line) {
            section = Some(match section_name.split_once('.') {
                Some((provider, service)) => Section::Service {
                    provider: provider.to_string(),
                    service: service.to_string(),
                },
                None => Section::Provider(section_name.to_string()),
            });
            continue;
        }

        let (key, value) = parse_key_value(line)
            .with_context(|| format!("invalid providers line {}", line_no + 1))?;

        match section.as_ref() {
            Some(Section::Provider(provider)) => {
                let entry = providers.entry(provider.clone()).or_default();
                if key == "api_key" {
                    entry.api_key = parse_string(value)?.to_string();
                }
            }
            Some(Section::Service { provider, service }) => {
                let service_entry = providers
                    .entry(provider.clone())
                    .or_default()
                    .services
                    .entry(service.clone())
                    .or_default();
                match key {
                    "base_url" => service_entry.base_url = Some(parse_string(value)?.to_string()),
                    "model" => service_entry.model = Some(parse_string(value)?.to_string()),
                    "dimension" => service_entry.dimension = Some(value.parse::<usize>()?),
                    "timeout_ms" => service_entry.timeout_ms = Some(value.parse::<u64>()?),
                    "max_concurrent" => {
                        service_entry.max_concurrent = Some(value.parse::<usize>()?)
                    }
                    _ => {}
                }
            }
            None => {
                anyhow::bail!("key-value pair found before any section header");
            }
        }
    }

    Ok(providers)
}

fn strip_comments(line: &str) -> &str {
    line.split('#').next().unwrap_or("")
}

fn parse_section_header(line: &str) -> Option<&str> {
    line.strip_prefix('[')?.strip_suffix(']')
}

fn parse_key_value(line: &str) -> Result<(&str, &str)> {
    let (key, value) = line
        .split_once('=')
        .with_context(|| format!("expected `key = value`, got `{line}`"))?;
    Ok((key.trim(), value.trim()))
}

fn parse_string(value: &str) -> Result<&str> {
    value
        .strip_prefix('"')
        .and_then(|item| item.strip_suffix('"'))
        .with_context(|| format!("expected quoted string, got `{value}`"))
}

fn parse_bool(value: &str) -> Result<bool> {
    match value.trim() {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => anyhow::bail!("expected bool, got `{value}`"),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use anyhow::Result;
    use tempfile::TempDir;

    use super::{
        build_engine_config, initialize_app_home, parse_app_config, parse_providers_config,
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

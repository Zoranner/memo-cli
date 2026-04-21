use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::{Context, Result};
use lmkit::{Provider, ProviderConfig};
use memo_engine::EngineConfig;

use crate::lmkit_adapter::LmkitEmbeddingAdapter;
use crate::lmkit_extraction_adapter::{ExtractionCleanupOptions, LmkitExtractionAdapter};
use crate::lmkit_rerank_adapter::LmkitRerankAdapter;

const CONFIG_TEMPLATE: &str = include_str!("templates/config.toml");
const PROVIDERS_TEMPLATE: &str = include_str!("templates/providers.toml");

#[derive(Debug, Default)]
struct EmbedConfig {
    embedding_provider: Option<String>,
    duplicate_threshold: Option<f32>,
}

#[derive(Debug, Default)]
struct ExtractConfig {
    extraction_provider: Option<String>,
    min_confidence: Option<f32>,
    normalize_predicates: Option<bool>,
}

#[derive(Debug, Default)]
struct RerankConfig {
    rerank_provider: Option<String>,
}

#[derive(Debug, Default)]
struct FileConfig {
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
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitReport {
    pub config_created: bool,
    pub providers_created: bool,
}

pub(crate) fn build_engine_config(data_dir: impl Into<PathBuf>) -> Result<EngineConfig> {
    let data_dir = data_dir.into();
    let mut engine_config = EngineConfig::new(&data_dir);
    let config_path = data_dir.join("config.toml");

    if !config_path.exists() {
        return Ok(engine_config);
    }

    let config_text = fs::read_to_string(&config_path)
        .with_context(|| format!("failed to read config file: {}", config_path.display()))?;
    let file_config = parse_app_config(&config_text)
        .with_context(|| format!("failed to parse config file: {}", config_path.display()))?;

    let _duplicate_threshold = file_config.embed.duplicate_threshold;

    if let Some(provider_ref) = file_config.embed.embedding_provider.as_deref() {
        let provider_config = load_provider_config(&data_dir, provider_ref, "embedding")?;
        let adapter = LmkitEmbeddingAdapter::new(provider_config)?;
        engine_config = engine_config.with_embedding_provider(Arc::new(adapter));
    }

    if let Some(provider_ref) = file_config.extract.extraction_provider.as_deref() {
        let provider_config = load_provider_config(&data_dir, provider_ref, "extraction")?;
        let adapter = LmkitExtractionAdapter::new_with_options(
            provider_config,
            extraction_cleanup_options(&file_config.extract),
        )?;
        engine_config = engine_config.with_extraction_provider(Arc::new(adapter));
    }

    if let Some(provider_ref) = file_config.rerank.rerank_provider.as_deref() {
        let provider_config = load_provider_config(&data_dir, provider_ref, "rerank")?;
        let adapter = LmkitRerankAdapter::new(provider_config)?;
        engine_config = engine_config.with_rerank_provider(Arc::new(adapter));
    }

    Ok(engine_config)
}

fn extraction_cleanup_options(config: &ExtractConfig) -> ExtractionCleanupOptions {
    ExtractionCleanupOptions {
        min_confidence: config.min_confidence.unwrap_or(0.5),
        normalize_predicates: config.normalize_predicates.unwrap_or(true),
    }
}

pub(crate) fn initialize_data_dir(data_dir: &Path) -> Result<InitReport> {
    fs::create_dir_all(data_dir)
        .with_context(|| format!("failed to create data dir: {}", data_dir.display()))?;

    let config_created = write_if_missing(&data_dir.join("config.toml"), CONFIG_TEMPLATE)?;
    let providers_created = write_if_missing(&data_dir.join("providers.toml"), PROVIDERS_TEMPLATE)?;

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

fn load_provider_config(
    data_dir: &Path,
    provider_ref: &str,
    capability: &str,
) -> Result<ProviderConfig> {
    let providers_path = data_dir.join("providers.toml");
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
            Some("embed") => match key {
                "embedding_provider" => {
                    config.embed.embedding_provider = Some(parse_string(value)?.to_string());
                }
                "duplicate_threshold" => {
                    config.embed.duplicate_threshold = Some(value.parse::<f32>()?);
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
                _ => {}
            },
            Some("rerank") => {
                if key == "rerank_provider" {
                    config.rerank.rerank_provider = Some(parse_string(value)?.to_string());
                }
            }
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

    use super::{build_engine_config, initialize_data_dir};

    #[test]
    fn init_writes_current_templates_into_data_dir() -> Result<()> {
        let temp = TempDir::new()?;

        let report = initialize_data_dir(temp.path())?;

        assert!(report.config_created);
        assert!(report.providers_created);
        assert!(temp.path().join("config.toml").exists());
        assert!(temp.path().join("providers.toml").exists());
        Ok(())
    }

    #[test]
    fn build_engine_config_loads_embedding_provider_from_local_files() -> Result<()> {
        let temp = TempDir::new()?;
        fs::write(
            temp.path().join("config.toml"),
            "[embed]\nembedding_provider = \"openai.embed\"\n[extract]\nextraction_provider = \"openai.extract\"\n",
        )?;
        fs::write(
            temp.path().join("providers.toml"),
            "[openai]\napi_key = \"sk-test\"\n[openai.embed]\nbase_url = \"https://api.openai.com/v1\"\nmodel = \"text-embedding-3-small\"\ndimension = 1536\n[openai.extract]\nbase_url = \"https://api.openai.com/v1\"\nmodel = \"gpt-4o-mini\"\n",
        )?;

        let config = build_engine_config(temp.path())?;

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
    fn build_engine_config_loads_rerank_provider_from_local_files() -> Result<()> {
        let temp = TempDir::new()?;
        fs::write(
            temp.path().join("config.toml"),
            "[rerank]\nrerank_provider = \"aliyun.rerank\"\n",
        )?;
        fs::write(
            temp.path().join("providers.toml"),
            "[aliyun]\napi_key = \"sk-test\"\n[aliyun.rerank]\nbase_url = \"https://dashscope.aliyuncs.com/api/v1\"\nmodel = \"gte-rerank\"\n",
        )?;

        let config = build_engine_config(temp.path())?;

        assert!(config.rerank_provider.is_some());
        Ok(())
    }

    #[test]
    fn build_engine_config_rejects_invalid_provider_ref() -> Result<()> {
        let temp = TempDir::new()?;
        fs::write(
            temp.path().join("config.toml"),
            "[embed]\nembedding_provider = \"openai\"\n",
        )?;
        fs::write(
            temp.path().join("providers.toml"),
            "[openai]\napi_key = \"sk-test\"\n",
        )?;

        let error = match build_engine_config(temp.path()) {
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
}

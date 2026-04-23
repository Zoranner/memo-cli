use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::{Context, Result};

#[derive(Debug, Default)]
pub(crate) struct EmbedConfig {
    pub(crate) embedding_provider: Option<String>,
    pub(crate) duplicate_threshold: Option<f32>,
    pub(crate) max_retries: Option<usize>,
    pub(crate) retry_backoff_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct ExtractConfig {
    pub(crate) extraction_provider: Option<String>,
    pub(crate) min_confidence: Option<f32>,
    pub(crate) normalize_predicates: Option<bool>,
    pub(crate) max_retries: Option<usize>,
    pub(crate) retry_backoff_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct RerankConfig {
    pub(crate) rerank_provider: Option<String>,
    pub(crate) max_retries: Option<usize>,
    pub(crate) retry_backoff_ms: Option<u64>,
}

#[derive(Debug, Default)]
pub(crate) struct StorageConfig {
    pub(crate) data_dir: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct FileConfig {
    pub(crate) storage: StorageConfig,
    pub(crate) embed: EmbedConfig,
    pub(crate) extract: ExtractConfig,
    pub(crate) rerank: RerankConfig,
}

pub(crate) fn load_file_config(config_dir: &Path) -> Result<Option<FileConfig>> {
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

pub(crate) fn resolve_relative_to_dir(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

pub(crate) fn parse_app_config(contents: &str) -> Result<FileConfig> {
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

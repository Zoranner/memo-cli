use std::{collections::HashMap, fs, path::Path, time::Duration};

use anyhow::{Context, Result};
use lmkit::{Provider, ProviderConfig};

#[derive(Debug, Default)]
pub(crate) struct ProviderEntry {
    pub(crate) api_key: String,
    pub(crate) services: HashMap<String, ProviderService>,
}

#[derive(Debug, Default)]
pub(crate) struct ProviderService {
    pub(crate) base_url: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) dimension: Option<usize>,
    pub(crate) timeout_ms: Option<u64>,
    pub(crate) max_concurrent: Option<usize>,
}

pub(crate) fn load_provider_config(
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

pub(crate) fn parse_providers_config(contents: &str) -> Result<HashMap<String, ProviderEntry>> {
    enum Section {
        Provider(String),
        Service { provider: String, service: String },
    }

    let mut providers = HashMap::<String, ProviderEntry>::new();
    let mut section: Option<Section> = None;

    for (line_no, raw_line) in contents.lines().enumerate() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }

        if let Some(section_name) = line
            .strip_prefix('[')
            .and_then(|item| item.strip_suffix(']'))
        {
            section = Some(match section_name.split_once('.') {
                Some((provider, service)) => Section::Service {
                    provider: provider.to_string(),
                    service: service.to_string(),
                },
                None => Section::Provider(section_name.to_string()),
            });
            continue;
        }

        let (key, value) = line
            .split_once('=')
            .with_context(|| format!("invalid providers line {}", line_no + 1))?;
        let key = key.trim();
        let value = value.trim();

        match section.as_ref() {
            Some(Section::Provider(provider)) => {
                let entry = providers.entry(provider.clone()).or_default();
                if key == "api_key" {
                    entry.api_key = value
                        .strip_prefix('"')
                        .and_then(|item| item.strip_suffix('"'))
                        .with_context(|| format!("expected quoted string, got `{value}`"))?
                        .to_string();
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
                    "base_url" => {
                        service_entry.base_url = Some(
                            value
                                .strip_prefix('"')
                                .and_then(|item| item.strip_suffix('"'))
                                .with_context(|| format!("expected quoted string, got `{value}`"))?
                                .to_string(),
                        )
                    }
                    "model" => {
                        service_entry.model = Some(
                            value
                                .strip_prefix('"')
                                .and_then(|item| item.strip_suffix('"'))
                                .with_context(|| format!("expected quoted string, got `{value}`"))?
                                .to_string(),
                        )
                    }
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

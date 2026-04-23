use std::path::Path;

use anyhow::{Context, Result};
use memo_engine::{EngineConfig, MemoryEngine};

use super::templates::{write_if_missing, CONFIG_TEMPLATE, PROVIDERS_TEMPLATE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct InitReport {
    pub config_created: bool,
    pub providers_created: bool,
}

pub(crate) fn initialize_app_home(config_dir: &Path, data_dir: &Path) -> Result<InitReport> {
    std::fs::create_dir_all(config_dir)
        .with_context(|| format!("failed to create config dir: {}", config_dir.display()))?;
    std::fs::create_dir_all(data_dir)
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

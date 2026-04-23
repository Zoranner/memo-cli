use std::{fs, path::Path};

use anyhow::{Context, Result};

pub(crate) const CONFIG_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/config.toml"
));
pub(crate) const PROVIDERS_TEMPLATE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/templates/providers.toml"
));

pub(crate) fn write_if_missing(path: &Path, contents: &str) -> Result<bool> {
    if path.exists() {
        return Ok(false);
    }

    fs::write(path, contents).with_context(|| format!("failed to write {}", path.display()))?;
    Ok(true)
}

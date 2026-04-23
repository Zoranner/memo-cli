use std::path::{Path, PathBuf};

use crate::config;
use anyhow::Result;

const MEMO_DATA_DIR_ENV: &str = "MEMO_DATA_DIR";

pub(crate) fn default_config_dir() -> Result<PathBuf> {
    Ok(user_home_dir()?.join(".memo"))
}

pub(crate) fn resolve_data_dir_for_config_dir(config_dir: &Path) -> Result<PathBuf> {
    if let Some(value) = std::env::var_os(MEMO_DATA_DIR_ENV) {
        return Ok(resolve_relative_to_dir(config_dir, Path::new(&value)));
    }

    if let Some(data_dir) = config::resolve_configured_data_dir(config_dir)? {
        return Ok(data_dir);
    }

    Ok(config_dir.join("data"))
}

fn user_home_dir() -> Result<PathBuf> {
    if let Some(value) = std::env::var_os("HOME").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(value));
    }

    if let Some(value) = std::env::var_os("USERPROFILE").filter(|value| !value.is_empty()) {
        return Ok(PathBuf::from(value));
    }

    if let (Some(drive), Some(path)) = (std::env::var_os("HOMEDRIVE"), std::env::var_os("HOMEPATH"))
    {
        let mut home = PathBuf::from(drive);
        home.push(path);
        if !home.as_os_str().is_empty() {
            return Ok(home);
        }
    }

    anyhow::bail!("failed to determine user home directory")
}

pub(crate) fn resolve_relative_to_dir(base_dir: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::resolve_data_dir_for_config_dir;

    #[test]
    fn resolve_data_dir_defaults_to_user_config_data_subdir() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");

        let resolved = resolve_data_dir_for_config_dir(&config_dir)?;

        assert_eq!(resolved, config_dir.join("data"));
        Ok(())
    }

    #[test]
    fn resolve_data_dir_uses_configured_data_dir_from_fixed_config_root() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[storage]\ndata_dir = \"memory-data\"\n",
        )?;

        let resolved = resolve_data_dir_for_config_dir(&config_dir)?;

        assert_eq!(resolved, config_dir.join("memory-data"));
        Ok(())
    }

    #[test]
    fn resolve_data_dir_prefers_environment_override_over_config() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let config_dir = temp.path().join(".memo");
        fs::create_dir_all(&config_dir)?;
        fs::write(
            config_dir.join("config.toml"),
            "[storage]\ndata_dir = \"memory-data\"\n",
        )?;
        unsafe {
            std::env::set_var("MEMO_DATA_DIR", "env-store");
        }

        let resolved = resolve_data_dir_for_config_dir(&config_dir)?;

        unsafe {
            std::env::remove_var("MEMO_DATA_DIR");
        }
        assert_eq!(resolved, config_dir.join("env-store"));
        Ok(())
    }
}

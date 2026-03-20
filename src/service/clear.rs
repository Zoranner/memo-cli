use anyhow::{Context, Result};

use crate::config::{AppConfig, ProvidersConfig};
use crate::service::storage_dim::resolve_storage_dimension;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};

/// 清空数据库（高危操作）
pub async fn clear(local: bool, global: bool, skip_confirm: bool) -> Result<()> {
    let output = Output::new();

    AppConfig::validate_scope_flags(local, global)?;

    let scope_name = AppConfig::get_scope_name(local, global);
    let config = if local {
        AppConfig::load_with_scope(true, false)?
    } else if global {
        AppConfig::load_with_scope(false, true)?
    } else {
        AppConfig::load()?
    };
    let brain_path = config.get_brain_path()?;

    if !brain_path.exists() {
        output.database_info(&brain_path, 0);
        output.info("Database is empty, nothing to clear.");
        return Ok(());
    }

    let providers = ProvidersConfig::load()?;
    let dimension = resolve_storage_dimension(&brain_path, &providers, &config)
        .with_context(|| format!("Cannot open database at {}", brain_path.display()))?;

    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension,
    };

    let storage = LocalStorageClient::connect(&storage_config)
        .await
        .with_context(|| format!("Cannot connect to database at {}", brain_path.display()))?;
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    output.warning("this will delete all memories");
    output.info(&format!(
        "{} database: {}",
        scope_name,
        brain_path.display()
    ));
    output.info(&format!("{} records will be deleted", record_count));

    if !skip_confirm && !output.confirm("yes")? {
        output.info("Operation cancelled");
        return Ok(());
    }

    output.begin_operation("Clearing", "database");

    storage.clear().await?;

    output.finish("clearing", scope_name);

    Ok(())
}

use anyhow::Result;

use crate::config::{AppConfig, ProvidersConfig};
use crate::service::storage_dim::resolve_storage_dimension;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};

pub async fn list(force_local: bool, force_global: bool) -> Result<()> {
    let output = Output::new();

    // 自动初始化
    let _initialized = crate::service::init::ensure_initialized().await?;

    let providers = ProvidersConfig::load()?;
    let config = AppConfig::load_with_scope(force_local, force_global)?;
    let brain_path = config.get_brain_path()?;
    let dimension = resolve_storage_dimension(&brain_path, &providers, &config)?;

    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension,
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;
    let record_count = storage.count().await?;

    // 显示数据库信息
    output.database_info(&brain_path, record_count);

    if record_count == 0 {
        output.info("No memories found. Use 'memo embed' to add some!");
        return Ok(());
    }

    // 列出所有记忆
    let results = storage.list().await?;

    // 显示结果
    output.list_results(&results);

    Ok(())
}

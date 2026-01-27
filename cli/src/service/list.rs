use anyhow::Result;

use crate::config::Config;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};

pub async fn list(force_local: bool, force_global: bool) -> Result<()> {
    let output = Output::new();

    // 自动初始化
    let _initialized = crate::service::init::ensure_initialized().await?;

    let config = Config::load_with_scope(force_local, force_global)?;

    // 创建存储客户端（list 不需要 embedding）
    let storage_config = StorageConfig {
        path: config.brain_path.to_string_lossy().to_string(),
        dimension: config.embedding_dimension.unwrap_or(1536),
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;
    let record_count = storage.count().await?;

    // 显示数据库信息
    output.database_info(&config.brain_path, record_count);

    if record_count == 0 {
        output.info("No memories found. Use 'memo embed' to add some!");
        return Ok(());
    }

    // 列出所有记忆
    let results = storage.list().await?;

    // 显示结果
    output.list_items(&results);

    Ok(())
}

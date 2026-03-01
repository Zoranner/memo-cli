use anyhow::Result;

use crate::config::AppConfig;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};

pub async fn list(force_local: bool, force_global: bool) -> Result<()> {
    let output = Output::new();

    // 自动初始化
    let _initialized = crate::service::init::ensure_initialized().await?;

    let config = AppConfig::load_with_scope(force_local, force_global)?;
    let brain_path = config.get_brain_path()?;

    // 创建存储客户端（list 不需要 embedding，使用默认维度）
    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension: 1536, // 默认维度，list 操作不依赖具体维度
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

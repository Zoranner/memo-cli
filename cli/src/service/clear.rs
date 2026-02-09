use anyhow::Result;

use crate::config::AppConfig;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};

/// 清空数据库（高危操作）
pub async fn clear(local: bool, global: bool, skip_confirm: bool) -> Result<()> {
    let output = Output::new();

    // 验证作用域标志
    AppConfig::validate_scope_flags(local, global)?;

    // 确定要清空的 brain 目录路径
    let scope_name = AppConfig::get_scope_name(local, global);
    let brain_path = if local {
        AppConfig::local_memo_dir().join("brain")
    } else if global {
        AppConfig::global_memo_dir().join("brain")
    } else {
        AppConfig::load()?.get_brain_path()?
    };

    // 检查数据库是否存在
    if !brain_path.exists() {
        output.database_info(&brain_path, 0);
        output.info("Database is empty, nothing to clear.");
        return Ok(());
    }

    // 尝试获取记录数
    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension: 1536, // 默认维度
    };

    let record_count = if let Ok(storage) = LocalStorageClient::connect(&storage_config).await {
        storage.count().await.unwrap_or(0)
    } else {
        0
    };

    // 显示数据库信息
    output.database_info(&brain_path, record_count);

    // 显示警告信息
    output.warning("this will delete all memories");
    output.info(&format!(
        "{} database: {}",
        scope_name,
        brain_path.display()
    ));
    output.info(&format!("{} records will be deleted", record_count));

    // 确认操作
    if !skip_confirm && !output.confirm("yes")? {
        output.info("Operation cancelled");
        return Ok(());
    }

    // 执行清空操作
    output.begin_operation("Clearing", "database");

    // 使用存储接口清空
    if let Ok(storage) = LocalStorageClient::connect(&storage_config).await {
        storage.clear().await?;
    }

    output.finish("clearing", scope_name);

    Ok(())
}

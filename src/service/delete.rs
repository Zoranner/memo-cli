use anyhow::Result;

use crate::config::AppConfig;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};

pub async fn delete(
    id: &str,
    force_local: bool,
    force_global: bool,
    skip_confirm: bool,
) -> Result<()> {
    let output = Output::new();

    let config = AppConfig::load_with_scope(force_local, force_global)?;
    let scope = AppConfig::get_scope_name(force_local, force_global);
    let brain_path = config.get_brain_path()?;

    // 创建存储客户端（delete 不需要 embedding）
    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension: 1536, // 默认维度
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    // 显示警告信息
    output.warning(&format!("this will permanently delete memory {}", id));

    // 确认操作
    if !skip_confirm && !output.confirm("yes")? {
        output.info("Operation cancelled");
        return Ok(());
    }

    // 删除记忆
    output.begin_operation("Deleting", &format!("memory {}", id));
    storage.delete(id).await?;

    output.finish("delete", scope);

    Ok(())
}

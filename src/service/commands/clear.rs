use anyhow::Result;

use crate::config::AppConfig;
use crate::service::session::{open_local_brain_from_config, BrainOpenMode};
use crate::ui::Output;
use memo_types::StorageBackend;

/// 清空数据库（高危操作）。仅打开本地存储，不初始化 Embedding API 客户端。
pub async fn clear(local: bool, global: bool, skip_confirm: bool) -> Result<()> {
    let output = Output::new();

    AppConfig::validate_scope_flags(local, global)?;

    let scope_name = AppConfig::get_scope_name(local, global);

    let config = AppConfig::load_with_scope(local, global)?;
    let brain_path = config.get_brain_path()?;

    if !brain_path.exists() {
        output.database_info(&brain_path, 0);
        output.info("Database is empty, nothing to clear.");
        return Ok(());
    }

    let (brain, _) = open_local_brain_from_config(config, BrainOpenMode::ClearExisting).await?;
    let storage = brain.storage;

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

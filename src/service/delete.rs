use anyhow::Result;

use crate::config::AppConfig;
use crate::service::context::{open_local_embed_session, LocalEmbedSession};
use crate::ui::Output;
use memo_types::StorageBackend;

pub async fn delete(
    id: &str,
    force_local: bool,
    force_global: bool,
    skip_confirm: bool,
) -> Result<()> {
    let output = Output::new();

    let (
        LocalEmbedSession {
            storage,
            brain_path,
            ..
        },
        _,
    ) = open_local_embed_session(force_local, force_global).await?;
    let scope = AppConfig::get_scope_name(force_local, force_global);
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

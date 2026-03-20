use anyhow::Result;

use crate::service::context::{open_local_embed_session, LocalEmbedSession};
use crate::ui::Output;
use memo_types::StorageBackend;

pub async fn list(force_local: bool, force_global: bool) -> Result<()> {
    let output = Output::new();

    let (
        LocalEmbedSession {
            storage,
            brain_path,
            ..
        },
        _,
    ) = open_local_embed_session(force_local, force_global).await?;
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

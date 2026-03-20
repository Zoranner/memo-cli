use anyhow::Result;

use crate::service::session::{open_local_embed_session, LocalEmbedSession};
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

    output.database_info(&brain_path, record_count);

    if record_count == 0 {
        output.info("No memories found. Use 'memo embed' to add some!");
        return Ok(());
    }

    let results = storage.list().await?;

    output.list_results(&results);

    Ok(())
}

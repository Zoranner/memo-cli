use anyhow::{Context, Result};

use crate::config::AppConfig;
use crate::service::session::{open_local_embed_session, LocalEmbedSession};
use crate::ui::Output;
use memo_types::StorageBackend;

pub async fn update(
    id: &str,
    content: String,
    tags: Option<Vec<String>>,
    force_local: bool,
    force_global: bool,
) -> Result<()> {
    let output = Output::new();

    let (
        LocalEmbedSession {
            storage,
            embed_provider,
            brain_path,
            ..
        },
        _,
    ) = open_local_embed_session(force_local, force_global).await?;
    let scope = AppConfig::get_scope_name(force_local, force_global);
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    output.status("Finding", &format!("memory {}", id));

    let old_memory = storage
        .find_by_id(id)
        .await?
        .with_context(|| format!("Memory not found with ID: {}", id))?;

    let final_tags = tags.unwrap_or(old_memory.tags);

    output.status("Encoding", "new content");
    let new_vector = embed_provider.encode(&content).await?;

    output.status("Updating", &format!("memory {}", id));
    storage.update(id, content, new_vector, final_tags).await?;

    output.finish("update", scope);

    Ok(())
}

use anyhow::{Context, Result};
use std::collections::HashSet;

use crate::config::AppConfig;
use crate::service::session::{open_local_embed_session, LocalEmbedSession};
use crate::ui::Output;
use memo_types::{Memory, MemoryBuilder, StorageBackend};

pub async fn merge(
    ids: Vec<String>,
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

    if ids.len() < 2 {
        return Err(output.fail("Need at least 2 memory IDs to merge"));
    }

    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    output.status("Collecting", &format!("{} memories", ids.len()));

    let mut all_tags = HashSet::new();
    let mut earliest_created_at = None;

    for id in &ids {
        let memory = storage
            .find_memory_by_id(id)
            .await?
            .with_context(|| format!("Memory not found with ID: {}", id))?;

        all_tags.extend(memory.tags);

        match earliest_created_at {
            None => earliest_created_at = Some(memory.created_at),
            Some(current) => {
                if memory.created_at < current {
                    earliest_created_at = Some(memory.created_at);
                }
            }
        }
    }

    let final_tags: Vec<String> = if let Some(user_tags) = tags {
        user_tags
    } else {
        all_tags.into_iter().collect()
    };

    output.status("Encoding", "merged content");
    let vector = embed_provider.encode(&content).await?;

    output.status("Merging", &format!("{} memories", ids.len()));

    let mut new_memory = Memory::new(MemoryBuilder {
        content,
        tags: final_tags,
        vector,
        source_file: None,
    });

    if let Some(earliest) = earliest_created_at {
        new_memory.created_at = earliest;
    }

    storage.insert(new_memory).await?;

    for id in &ids {
        storage.delete(id).await?;
    }

    output.finish("merge", scope);

    Ok(())
}

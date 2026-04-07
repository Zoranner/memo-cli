use anyhow::Result;

use crate::config::AppConfig;
use crate::service::session::{open_local_embed_session, LocalEmbedSession};
use crate::ui::Output;
use lmkit::EmbedProvider;
use memo_types::{Memory, MemoryBuilder, StorageBackend};

pub async fn embed(
    input: String,
    user_tags: Option<Vec<String>>,
    force: bool,
    dup_threshold: Option<f32>,
    force_local: bool,
    force_global: bool,
) -> Result<()> {
    let (
        LocalEmbedSession {
            config,
            storage,
            embed_provider,
            brain_path,
            embedding,
            ..
        },
        _,
    ) = open_local_embed_session(force_local, force_global).await?;

    let output = Output::new();
    let scope = AppConfig::get_scope_name(force_local, force_global);

    let record_count = storage.count().await?;

    output.database_info_with_model(
        &brain_path,
        record_count,
        &embedding.model,
        embed_provider.dimension(),
    );

    let duplicate_threshold = dup_threshold.unwrap_or(config.embed.duplicate_threshold);

    embed_text(
        &*embed_provider,
        &storage,
        &input,
        user_tags.as_ref(),
        force,
        duplicate_threshold,
    )
    .await?;

    output.finish("embedding", scope);

    Ok(())
}

async fn embed_text(
    model: &dyn EmbedProvider,
    storage: &dyn StorageBackend,
    text: &str,
    user_tags: Option<&Vec<String>>,
    force: bool,
    duplicate_threshold: f32,
) -> Result<()> {
    let output = Output::new();

    let normalized = normalize_for_embedding(text);
    let embedding = model.encode(&normalized).await?;

    check_duplicate_and_abort_if_found(storage, &embedding, duplicate_threshold, force).await?;

    let tags = user_tags.cloned().unwrap_or_default();

    let memory = Memory::new(MemoryBuilder {
        content: text.to_string(),
        tags,
        vector: embedding,
        source_file: None,
    });

    storage.insert(memory).await?;

    output.status("Embedded", "text");

    Ok(())
}

async fn check_duplicate_and_abort_if_found(
    storage: &dyn StorageBackend,
    vector: &[f32],
    threshold: f32,
    force: bool,
) -> Result<()> {
    if force {
        return Ok(());
    }

    let output = Output::new();
    output.status("Checking", "for similar memories");

    let similar_memories = storage
        .search_by_vector(vector.to_vec(), 5, threshold, None)
        .await?;

    if !similar_memories.is_empty() {
        output.warning(&format!(
            "Found {} similar memories (threshold: {:.2})",
            similar_memories.len(),
            threshold
        ));

        output.search_results_brief(&similar_memories);

        match similar_memories.len() {
            1 => {
                let id = &similar_memories[0].id;
                output.note(&format!(
                    "Consider updating the existing memory: memo update {}",
                    id
                ));
                output.note("Or delete it and add new: memo delete <id>, then embed again");
            }
            2 => {
                let id1 = &similar_memories[0].id;
                let id2 = &similar_memories[1].id;
                output.note(&format!(
                    "Consider merging similar memories: memo merge {} {}",
                    id1, id2
                ));
                output.note("Or update the most relevant one: memo update <id>");
            }
            _ => {
                output.note("Consider reorganizing memories:");
                output.note("  - Merge overlapping content: memo merge <id1> <id2> ...");
                output.note("  - Update the most relevant one: memo update <id>");
                output.note("  - Delete outdated ones: memo delete <id>");
            }
        }

        output.note("Or use --force to add anyway (not recommended)");
        return Err(output.fail(
            "Embedding cancelled: similar memories found above threshold (use `memo update` / `memo merge`, or `--force` to add anyway)",
        ));
    }

    Ok(())
}

fn normalize_for_embedding(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

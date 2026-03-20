use anyhow::{Context, Result};

use crate::config::AppConfig;
use crate::parser::parse_markdown_file;
use crate::service::session::{open_local_embed_session, LocalEmbedSession};
use crate::ui::Output;
use memo_types::{Memory, MemoryBuilder, StorageBackend};
use model_provider::EmbedProvider;
use walkdir::WalkDir;

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

    let duplicate_threshold = dup_threshold.unwrap_or(config.duplicate_threshold);

    let expanded_input = shellexpand::tilde(&input).to_string();
    let input_path = std::path::Path::new(&expanded_input);

    if input_path.exists() {
        if input_path.is_dir() {
            embed_directory(
                &*embed_provider,
                &storage,
                input_path,
                user_tags.as_ref(),
                force,
                duplicate_threshold,
            )
            .await?;
        } else if input_path.is_file() {
            embed_file(
                &*embed_provider,
                &storage,
                input_path,
                user_tags.as_ref(),
                force,
                duplicate_threshold,
            )
            .await?;
        }
    } else {
        embed_text(
            &*embed_provider,
            &storage,
            &input,
            user_tags.as_ref(),
            force,
            duplicate_threshold,
        )
        .await?;
    }

    output.finish("embedding", scope);

    Ok(())
}

async fn embed_directory(
    model: &dyn EmbedProvider,
    storage: &dyn StorageBackend,
    dir_path: &std::path::Path,
    user_tags: Option<&Vec<String>>,
    force: bool,
    duplicate_threshold: f32,
) -> Result<()> {
    let output = Output::new();
    let mut total_files = 0;
    let mut total_sections = 0;

    for entry in WalkDir::new(dir_path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        total_files += 1;
        let file_path = entry.path();

        let sections = parse_markdown_file(file_path)
            .with_context(|| format!("Failed to parse file: {}", file_path.display()))?;

        for section in sections {
            output.status("Embedding", &file_path.display().to_string());
            embed_section(
                model,
                storage,
                section,
                Some(file_path),
                user_tags,
                force,
                duplicate_threshold,
            )
            .await?;
            total_sections += 1;
        }
    }

    output.stats(&[("files", total_files), ("sections", total_sections)]);

    Ok(())
}

async fn embed_file(
    model: &dyn EmbedProvider,
    storage: &dyn StorageBackend,
    file_path: &std::path::Path,
    user_tags: Option<&Vec<String>>,
    force: bool,
    duplicate_threshold: f32,
) -> Result<()> {
    let output = Output::new();

    let sections = parse_markdown_file(file_path)
        .with_context(|| format!("Failed to parse file: {}", file_path.display()))?;

    let total_sections = sections.len();

    for section in sections {
        output.status("Embedding", &file_path.display().to_string());
        embed_section(
            model,
            storage,
            section,
            Some(file_path),
            user_tags,
            force,
            duplicate_threshold,
        )
        .await?;
    }

    output.stats(&[("sections", total_sections)]);

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

async fn embed_section(
    model: &dyn EmbedProvider,
    storage: &dyn StorageBackend,
    section: memo_types::MemoSection,
    file_path: Option<&std::path::Path>,
    user_tags: Option<&Vec<String>>,
    force: bool,
    duplicate_threshold: f32,
) -> Result<()> {
    let normalized = normalize_for_embedding(&section.content);
    let embedding = model.encode(&normalized).await?;

    check_duplicate_and_abort_if_found(storage, &embedding, duplicate_threshold, force).await?;

    let mut tags = section.metadata.tags;
    if let Some(user_tags) = user_tags {
        for tag in user_tags {
            if !tags.contains(tag) {
                tags.push(tag.clone());
            }
        }
    }

    let memory = Memory::new(MemoryBuilder {
        content: section.content,
        tags,
        vector: embedding,
        source_file: file_path.map(|p| p.to_string_lossy().to_string()),
    });

    storage.insert(memory).await?;

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

        output.search_results(&similar_memories);

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

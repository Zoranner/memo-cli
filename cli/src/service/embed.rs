use anyhow::{Context, Result};

use crate::config::Config;
use crate::embedding::EmbeddingModel;
use crate::parser::parse_markdown_file;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{Memory, MemoryBuilder, StorageBackend, StorageConfig};
use walkdir::WalkDir;

pub async fn embed(
    input: String,
    user_tags: Option<Vec<String>>,
    force: bool,
    dup_threshold: Option<f32>,
    force_local: bool,
    force_global: bool,
) -> Result<()> {
    // 自动初始化
    let _initialized = crate::service::init::ensure_initialized().await?;

    let output = Output::new();
    let config = Config::load_with_scope(force_local, force_global)?;
    let scope = Config::get_scope_name(force_local, force_global);

    // 检查 API key（Ollama 不需要）
    config.validate_api_key(force_local)?;

    // 创建 embedding 模型
    let model = EmbeddingModel::new(
        config.embedding_api_key.clone(),
        config.embedding_model.clone(),
        config.embedding_base_url.clone(),
        config.embedding_dimension,
        config.embedding_provider.clone(),
    )?;

    // 创建存储客户端
    let storage_config = StorageConfig {
        path: config.brain_path.to_string_lossy().to_string(),
        dimension: model.dimension(),
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;
    let record_count = storage.count().await?;

    // 显示数据库信息（包含模型和维度）
    output.database_info_with_model(
        &config.brain_path,
        record_count,
        &config.embedding_model,
        model.dimension(),
    );

    // 使用命令行参数或配置文件中的阈值
    let duplicate_threshold = dup_threshold.unwrap_or(config.duplicate_threshold);

    let expanded_input = shellexpand::tilde(&input).to_string();
    let input_path = std::path::Path::new(&expanded_input);

    // 智能检测输入类型
    if input_path.exists() {
        if input_path.is_dir() {
            // 情况1：目录 - 递归扫描所有 .md 文件
            embed_directory(
                &model,
                &storage,
                input_path,
                user_tags.as_ref(),
                force,
                duplicate_threshold,
            )
            .await?;
        } else if input_path.is_file() {
            // 情况2：单个文件
            embed_file(
                &model,
                &storage,
                input_path,
                user_tags.as_ref(),
                force,
                duplicate_threshold,
            )
            .await?;
        }
    } else {
        // 情况3：纯文本字符串
        embed_text(
            &model,
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

/// 嵌入目录中的所有 markdown 文件
async fn embed_directory(
    model: &EmbeddingModel,
    storage: &LocalStorageClient,
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

/// 嵌入单个 markdown 文件
async fn embed_file(
    model: &EmbeddingModel,
    storage: &LocalStorageClient,
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

/// 规范化文本用于嵌入（移除多余空白符，提高匹配一致性）
fn normalize_for_embedding(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 检查重复记忆，如果发现则终止程序
/// 返回 Ok(()) 表示无重复，可以继续嵌入
async fn check_duplicate_and_abort_if_found(
    storage: &LocalStorageClient,
    vector: &[f32],
    threshold: f32,
    force: bool,
) -> Result<()> {
    // 如果是强制模式，跳过检查
    if force {
        return Ok(());
    }

    let output = Output::new();
    output.status("Checking", "for similar memories");

    // 使用向量搜索检查相似记忆
    let similar_memories = storage
        .search_by_vector(vector.to_vec(), 5, threshold, None)
        .await?;

    if !similar_memories.is_empty() {
        // 检测到相似记忆，输出详细信息并取消嵌入
        output.warning(&format!(
            "Found {} similar memories (threshold: {:.2})",
            similar_memories.len(),
            threshold
        ));

        // 显示相似记忆
        output.search_results(&similar_memories);

        // 根据相似记忆数量提供更具体的建议
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
        output.error("Embedding cancelled due to similar memories");

        std::process::exit(1);
    }

    Ok(())
}

/// 嵌入纯文本字符串
async fn embed_text(
    model: &EmbeddingModel,
    storage: &LocalStorageClient,
    text: &str,
    user_tags: Option<&Vec<String>>,
    force: bool,
    duplicate_threshold: f32,
) -> Result<()> {
    let output = Output::new();

    // 规范化文本用于嵌入
    let normalized = normalize_for_embedding(text);
    let embedding = model.encode(&normalized).await?;

    // 检查重复
    check_duplicate_and_abort_if_found(storage, &embedding, duplicate_threshold, force).await?;

    // 使用用户提供的 tags，如果没有则为空数组
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

/// 嵌入单个 section
async fn embed_section(
    model: &EmbeddingModel,
    storage: &LocalStorageClient,
    section: memo_types::MemoSection,
    file_path: Option<&std::path::Path>,
    user_tags: Option<&Vec<String>>,
    force: bool,
    duplicate_threshold: f32,
) -> Result<()> {
    // 规范化文本用于嵌入
    let normalized = normalize_for_embedding(&section.content);
    let embedding = model.encode(&normalized).await?;

    // 检查重复
    check_duplicate_and_abort_if_found(storage, &embedding, duplicate_threshold, force).await?;

    // 合并 frontmatter tags 和用户提供的 tags（去重）
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

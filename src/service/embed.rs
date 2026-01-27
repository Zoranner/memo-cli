use anyhow::{Context, Result};
use arrow_array::{
    ArrayRef, RecordBatch, RecordBatchIterator, StringArray, TimestampMillisecondArray,
};
use std::sync::Arc;

use crate::config::Config;
use crate::db::{Connection, TableOperations};
use crate::embedding::EmbeddingModel;
use crate::models::{Memory, MemoryBuilder};
use crate::parser::parse_markdown_file;
use crate::ui::Output;
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

    // 连接数据库并显示基本信息
    let conn = Connection::connect(&config.brain_path).await?;

    // 获取数据库信息
    let table_exists = TableOperations::table_exists(conn.inner(), "memories").await;
    let record_count = if table_exists {
        let table = TableOperations::open_table(conn.inner(), "memories").await?;
        table.count_rows(None).await.unwrap_or(0)
    } else {
        0
    };

    // 检查 API key（Ollama 不需要）
    config.validate_api_key(force_local)?;

    let model = EmbeddingModel::new(
        config.embedding_api_key.clone(),
        config.embedding_model.clone(),
        config.embedding_base_url.clone(),
        config.embedding_dimension,
        config.embedding_provider.clone(),
    )?;

    // 显示数据库信息（包含模型和维度）
    output.database_info_with_model(
        &config.brain_path,
        record_count,
        &config.embedding_model,
        model.dimension(),
    );

    let table = if table_exists {
        TableOperations::open_table(conn.inner(), "memories").await?
    } else {
        TableOperations::create_table(conn.inner(), "memories").await?
    };

    // 使用命令行参数或配置文件中的阈值
    let duplicate_threshold = dup_threshold.unwrap_or(config.duplicate_threshold);

    let expanded_input = shellexpand::tilde(&input).to_string();
    let input_path = std::path::Path::new(&expanded_input);

    // 智能检测输入类型
    if input_path.exists() {
        if input_path.is_dir() {
            // 情况1：目录 - 递归扫描所有 .md 文件
            embed_directory(&model, &table, user_tags.as_ref()).await?;
        } else if input_path.is_file() {
            // 情况2：单个文件
            embed_file(&model, &table, input_path, user_tags.as_ref()).await?;
        }
    } else {
        // 情况3：纯文本字符串
        embed_text(
            &model,
            &table,
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
    table: &lancedb::Table,
    user_tags: Option<&Vec<String>>,
) -> Result<()> {
    let output = Output::new();
    let mut total_files = 0;
    let mut total_sections = 0;

    for entry in WalkDir::new(".")
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
            embed_section(model, table, section, Some(file_path), user_tags).await?;
            total_sections += 1;
        }
    }

    output.stats(&[("files", total_files), ("sections", total_sections)]);

    Ok(())
}

/// 嵌入单个 markdown 文件
async fn embed_file(
    model: &EmbeddingModel,
    table: &lancedb::Table,
    file_path: &std::path::Path,
    user_tags: Option<&Vec<String>>,
) -> Result<()> {
    let output = Output::new();

    let sections = parse_markdown_file(file_path)
        .with_context(|| format!("Failed to parse file: {}", file_path.display()))?;

    let total_sections = sections.len();

    for section in sections {
        output.status("Embedding", &file_path.display().to_string());
        embed_section(model, table, section, Some(file_path), user_tags).await?;
    }

    output.stats(&[("sections", total_sections)]);

    Ok(())
}

/// 规范化文本用于嵌入（移除多余空白符，提高匹配一致性）
fn normalize_for_embedding(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// 嵌入纯文本字符串
async fn embed_text(
    model: &EmbeddingModel,
    table: &lancedb::Table,
    text: &str,
    user_tags: Option<&Vec<String>>,
    force: bool,
    duplicate_threshold: f32,
) -> Result<()> {
    let output = Output::new();

    // 规范化文本用于嵌入
    let normalized = normalize_for_embedding(text);
    let embedding = model.encode(&normalized).await?;

    // 如果不是强制模式，检查是否有相似的记忆
    if !force {
        output.status("Checking", "for similar memories");

        use futures::TryStreamExt;
        use lancedb::query::{ExecutableQuery, QueryBase};

        let mut stream = table
            .vector_search(embedding.clone())?
            .select(lancedb::query::Select::columns(&[
                "id",
                "content",
                "tags",
                "updated_at",
                "_distance",
            ]))
            .limit(5)
            .execute()
            .await?;

        if let Some(batch) = stream.try_next().await? {
            let distances = batch
                .column_by_name("_distance")
                .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>())
                .context("Failed to get distance column")?;

            let mut similar_memories = Vec::new();

            for i in 0..batch.num_rows() {
                let distance = distances.value(i);
                let score = (1.0 - (distance / 2.0)).max(0.0);

                if score >= duplicate_threshold {
                    let ids = batch
                        .column_by_name("id")
                        .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
                        .context("Failed to get id column")?;

                    let contents = batch
                        .column_by_name("content")
                        .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
                        .context("Failed to get content column")?;

                    let tags_col = batch
                        .column_by_name("tags")
                        .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
                        .context("Failed to get tags column")?;

                    let updated_ats = batch
                        .column_by_name("updated_at")
                        .and_then(|c| {
                            c.as_any()
                                .downcast_ref::<arrow_array::TimestampMillisecondArray>()
                        })
                        .context("Failed to get updated_at column")?;

                    let id = ids.value(i).to_string();
                    let content = contents.value(i).to_string();
                    let tags: Vec<String> = serde_json::from_str(tags_col.value(i))?;
                    let timestamp = updated_ats.value(i);
                    let updated = chrono::DateTime::from_timestamp_millis(timestamp)
                        .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
                        .unwrap_or_else(|| "N/A".to_string());

                    similar_memories.push((score, id, content, tags, updated));
                }
            }

            if !similar_memories.is_empty() {
                // 检测到相似记忆，输出详细信息并取消嵌入
                output.warning(&format!(
                    "Found {} similar memories (threshold: {:.2})",
                    similar_memories.len(),
                    duplicate_threshold
                ));

                for (i, (score, id, content, _tags, updated)) in similar_memories.iter().enumerate()
                {
                    output.search_result(*score, id, updated, content);

                    // 如果不是最后一个结果，添加空行分隔
                    if i < similar_memories.len() - 1 {
                        println!();
                    }
                }

                println!();
                
                // 根据相似记忆数量提供更具体的建议
                match similar_memories.len() {
                    1 => {
                        let id = &similar_memories[0].1;
                        output.note(&format!("Consider updating the existing memory: memo update {}", id));
                        output.note("Or delete it and add new: memo delete <id>, then embed again");
                    }
                    2 => {
                        let id1 = &similar_memories[0].1;
                        let id2 = &similar_memories[1].1;
                        output.note(&format!("Consider merging similar memories: memo merge {} {}", id1, id2));
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
        }
    }

    // 使用用户提供的 tags，如果没有则为空数组
    let tags = user_tags.cloned().unwrap_or_default();

    let memory = Memory::new(MemoryBuilder {
        content: text.to_string(),
        tags,
        vector: embedding,
        source_file: None,
    });

    let batch = memory_to_batch(&memory)?;
    let schema = crate::models::memory_schema();
    let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);
    table.add(Box::new(batches)).execute().await?;

    output.status("Embedded", "text");

    Ok(())
}

/// 嵌入单个 section
async fn embed_section(
    model: &EmbeddingModel,
    table: &lancedb::Table,
    section: crate::models::MemoSection,
    file_path: Option<&std::path::Path>,
    user_tags: Option<&Vec<String>>,
) -> Result<()> {
    // 规范化文本用于嵌入
    let normalized = normalize_for_embedding(&section.content);
    let embedding = model.encode(&normalized).await?;

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

    let batch = memory_to_batch(&memory)?;
    let schema = crate::models::memory_schema();
    let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);
    table.add(Box::new(batches)).execute().await?;

    Ok(())
}

fn memory_to_batch(memory: &Memory) -> Result<RecordBatch> {
    let id_array = StringArray::from(vec![memory.id.as_str()]);
    let content_array = StringArray::from(vec![memory.content.as_str()]);
    let tags_json = serde_json::to_string(&memory.tags)?;
    let tags_array = StringArray::from(vec![tags_json.as_str()]);
    let source_file_array = StringArray::from(vec![memory.source_file.as_deref().unwrap_or("")]);

    let vector_values: Vec<Option<f32>> = memory.vector.iter().map(|&v| Some(v)).collect();
    let vector_array = arrow_array::FixedSizeListArray::from_iter_primitive::<
        arrow_array::types::Float32Type,
        _,
        _,
    >(vec![Some(vector_values)], 1024);

    let created_at_ms = memory.created_at.timestamp_millis();
    let updated_at_ms = memory.updated_at.timestamp_millis();

    let created_at_array = TimestampMillisecondArray::from(vec![Some(created_at_ms)]);
    let updated_at_array = TimestampMillisecondArray::from(vec![Some(updated_at_ms)]);

    RecordBatch::try_new(
        crate::models::memory_schema(),
        vec![
            Arc::new(id_array) as ArrayRef,
            Arc::new(content_array) as ArrayRef,
            Arc::new(tags_array) as ArrayRef,
            Arc::new(vector_array) as ArrayRef,
            Arc::new(source_file_array) as ArrayRef,
            Arc::new(created_at_array) as ArrayRef,
            Arc::new(updated_at_array) as ArrayRef,
        ],
    )
    .context("Failed to create RecordBatch")
}

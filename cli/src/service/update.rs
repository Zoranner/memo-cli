use anyhow::{Context, Result};

use crate::config::Config;
use crate::embedding::EmbeddingModel;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};

pub async fn update(
    id: &str,
    content: String,
    tags: Option<Vec<String>>,
    force_local: bool,
    force_global: bool,
) -> Result<()> {
    let output = Output::new();
    let config = Config::load_with_scope(force_local, force_global)?;
    let scope = Config::get_scope_name(force_local, force_global);

    // 检查 API key
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

    output.database_info(&config.brain_path, record_count);

    // 查找要更新的记忆
    output.status("Finding", &format!("memory {}", id));

    let old_memory = storage
        .find_by_id(id)
        .await?
        .with_context(|| format!("Memory not found with ID: {}", id))?;

    // 使用新 tags 或保留原有 tags
    let final_tags = tags.unwrap_or(old_memory.tags);

    // 编码新内容
    output.status("Encoding", "new content");
    let new_vector = model.encode(&content).await?;

    // 更新记忆
    output.status("Updating", &format!("memory {}", id));
    storage.update(id, content, new_vector, final_tags).await?;

    output.finish("update", scope);

    Ok(())
}

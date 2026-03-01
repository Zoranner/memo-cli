use anyhow::{Context, Result};

use crate::config::{AppConfig, ProvidersConfig};
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig};
use model_provider::create_embed_provider;

pub async fn update(
    id: &str,
    content: String,
    tags: Option<Vec<String>>,
    force_local: bool,
    force_global: bool,
) -> Result<()> {
    let output = Output::new();

    // 加载 providers 和 app 配置
    let providers = ProvidersConfig::load()?;
    let config = AppConfig::load_with_scope(force_local, force_global)?;
    let scope = AppConfig::get_scope_name(force_local, force_global);

    // 解析 embedding 服务配置
    let embed_config = config.resolve_embedding(&providers)?;
    let dimension = embed_config.get_int("dimension").unwrap() as usize;
    let provider_config = embed_config.to_provider_config(Some(dimension));
    let embed_provider = create_embed_provider(&provider_config)?;

    // 创建存储客户端
    let brain_path = config.get_brain_path()?;
    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension: embed_provider.dimension(),
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

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
    let new_vector = embed_provider.encode(&content).await?;

    // 更新记忆
    output.status("Updating", &format!("memory {}", id));
    storage.update(id, content, new_vector, final_tags).await?;

    output.finish("update", scope);

    Ok(())
}

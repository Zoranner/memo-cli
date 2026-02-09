use anyhow::{Context, Result};
use std::collections::HashSet;

use crate::config::{AppConfig, ProvidersConfig};
use crate::providers::create_embed_provider;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{Memory, MemoryBuilder, StorageBackend, StorageConfig};

pub async fn merge(
    ids: Vec<String>,
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

    if ids.len() < 2 {
        anyhow::bail!("Need at least 2 memory IDs to merge");
    }

    // 解析 embedding 服务配置
    let embed_config = config.resolve_embedding(&providers)?;
    let embed_provider = create_embed_provider(&embed_config)?;

    // 创建存储客户端
    let brain_path = config.get_brain_path()?;
    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension: embed_provider.dimension(),
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    // 验证所有记忆是否存在，并收集信息
    output.status("Collecting", &format!("{} memories", ids.len()));

    let mut all_tags = HashSet::new();
    let mut earliest_created_at = None;

    for id in &ids {
        let query_result = storage
            .find_by_id(id)
            .await?
            .with_context(|| format!("Memory not found with ID: {}", id))?;

        // 合并 tags（自动去重）
        all_tags.extend(query_result.tags);

        // 获取完整的记忆以访问 created_at
        let memory = storage
            .find_memory_by_id(id)
            .await?
            .with_context(|| format!("Failed to get full memory: {}", id))?;

        // 找到最早的 created_at
        match earliest_created_at {
            None => earliest_created_at = Some(memory.created_at),
            Some(current) => {
                if memory.created_at < current {
                    earliest_created_at = Some(memory.created_at);
                }
            }
        }
    }

    // 使用用户提供的 tags 或合并后的 tags
    let final_tags: Vec<String> = if let Some(user_tags) = tags {
        user_tags
    } else {
        all_tags.into_iter().collect()
    };

    // 编码合并后的内容
    output.status("Encoding", "merged content");
    let vector = embed_provider.encode(&content).await?;

    // 插入合并后的新记忆（保留最早的 created_at）
    output.status("Merging", &format!("{} memories", ids.len()));

    let mut new_memory = Memory::new(MemoryBuilder {
        content,
        tags: final_tags,
        vector,
        source_file: None,
    });

    // 保留最早的 created_at
    if let Some(earliest) = earliest_created_at {
        new_memory.created_at = earliest;
    }

    storage.insert(new_memory).await?;

    // 删除旧记忆
    for id in &ids {
        storage.delete(id).await?;
    }

    output.finish("merge", scope);

    Ok(())
}

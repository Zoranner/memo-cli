//! 按与命令一致的作用域打开配置、嵌入服务与本地存储，避免「自动 ensure」与 `--local`/`--global` 错位。

use anyhow::Result;
use std::path::PathBuf;

use crate::config::{AppConfig, ProvidersConfig};
use memo_local::{DatabaseMetadata, LocalStorageClient};
use memo_types::{StorageBackend, StorageConfig};
use model_provider::{create_embed_provider, EmbedProvider};

use super::storage_dim::resolve_storage_dimension;

pub struct LocalEmbedSession {
    pub config: AppConfig,
    pub storage: LocalStorageClient,
    pub embed_provider: Box<dyn EmbedProvider>,
    pub brain_path: PathBuf,
}

/// 加载配置、连接 Lance 表（不存在则初始化并写入 metadata），维度优先与已有 `metadata.json` 一致。
pub async fn open_local_embed_session(
    force_local: bool,
    force_global: bool,
) -> Result<(LocalEmbedSession, bool)> {
    let config = AppConfig::load_with_scope(force_local, force_global)?;
    config.ensure_dirs()?;

    let providers = ProvidersConfig::load()?;
    let brain_path = config.get_brain_path()?;
    let dimension = resolve_storage_dimension(&brain_path, &providers, &config)?;
    let embed_config = config.resolve_embedding(&providers)?;
    let provider_config = embed_config.to_provider_config(Some(dimension));
    let embed_provider = create_embed_provider(&provider_config)?;

    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension,
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;

    let mut created = false;
    if !storage.exists().await? {
        storage.init().await?;
        let metadata = DatabaseMetadata::new(embed_config.model.clone(), dimension);
        metadata.save(&brain_path)?;
        created = true;
    }

    Ok((
        LocalEmbedSession {
            config,
            storage,
            embed_provider,
            brain_path,
        },
        created,
    ))
}

//! 按 CLI 作用域打开配置与本地 Lance 存储；可选再挂载 Embedding Provider。
//!
//! `open_local_brain` 仅连接存储（用于 clear 等无需向量化 API 的操作）。
//! `open_local_embed_session` 在 brain 就绪后创建 embed provider，供 embed / search / merge 等使用。

use anyhow::Result;
use std::path::PathBuf;

use crate::config::{AppConfig, ProvidersConfig, ResolvedService};
use memo_local::{DatabaseMetadata, LocalStorageClient};
use memo_types::{StorageBackend, StorageConfig};
use model_provider::{create_embed_provider, EmbedProvider};

use super::storage_dim::resolve_storage_dimension;

/// 已解析的本地 brain：配置、存储、embedding 服务描述（尚未创建 HTTP embed 客户端）。
pub struct LocalBrainSession {
    pub config: AppConfig,
    pub providers: ProvidersConfig,
    pub storage: LocalStorageClient,
    pub brain_path: PathBuf,
    pub embedding: ResolvedService,
    pub dimension: usize,
}

/// 在 [`LocalBrainSession`] 基础上挂载可向量化文本的 Provider。
pub struct LocalEmbedSession {
    pub config: AppConfig,
    pub providers: ProvidersConfig,
    pub storage: LocalStorageClient,
    pub embed_provider: Box<dyn EmbedProvider>,
    pub brain_path: PathBuf,
    /// 与当前 brain / provider 一致的 embedding 服务解析结果（展示模型名等，无需再 `resolve_embedding`）
    pub embedding: ResolvedService,
}

/// 加载配置并打开 brain（写入路径：创建目录、表不存在则初始化）。
pub async fn open_local_brain(
    force_local: bool,
    force_global: bool,
) -> Result<(LocalBrainSession, bool)> {
    let config = AppConfig::load_with_scope(force_local, force_global)?;
    open_local_brain_from_config(config, BrainOpenMode::Write).await
}

/// 在已有 `AppConfig` 上打开 brain，可控制是否创建目录 / 是否初始化空表。
///
/// - [`BrainOpenMode::Write`]：`embed` / `search` 等；`ensure_dirs` + 表缺失时 `init`。
/// - [`BrainOpenMode::ClearExisting`]：`clear` 专用；不创建目录、不初始化空库（避免「清空」反而建表）。
pub async fn open_local_brain_from_config(
    config: AppConfig,
    mode: BrainOpenMode,
) -> Result<(LocalBrainSession, bool)> {
    if mode.ensure_dirs() {
        config.ensure_dirs()?;
    }

    let providers = ProvidersConfig::load()?;
    let brain_path = config.get_brain_path()?;
    let dimension = resolve_storage_dimension(&brain_path, &providers, &config)?;
    let embedding = config.resolve_embedding(&providers)?;

    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension,
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;

    let mut created = false;
    if !storage.exists().await? && mode.init_if_missing() {
        storage.init().await?;
        let metadata = DatabaseMetadata::new(embedding.model.clone(), dimension);
        metadata.save(&brain_path)?;
        created = true;
    }

    Ok((
        LocalBrainSession {
            config,
            providers,
            storage,
            brain_path,
            embedding,
            dimension,
        },
        created,
    ))
}

#[derive(Clone, Copy)]
pub enum BrainOpenMode {
    /// 正常写入：创建 brain 目录，表不存在则初始化。
    Write,
    /// 清空命令：调用方已确认 brain 目录存在；不 `ensure_dirs`、不初始化空表。
    ClearExisting,
}

impl BrainOpenMode {
    fn ensure_dirs(self) -> bool {
        matches!(self, Self::Write)
    }

    fn init_if_missing(self) -> bool {
        matches!(self, Self::Write)
    }
}

/// 在 [`open_local_brain`] 之后创建 Embedding Provider。
pub async fn open_local_embed_session(
    force_local: bool,
    force_global: bool,
) -> Result<(LocalEmbedSession, bool)> {
    let (brain, created) = open_local_brain(force_local, force_global).await?;
    attach_embed_provider(brain, created)
}

fn attach_embed_provider(
    brain: LocalBrainSession,
    created: bool,
) -> Result<(LocalEmbedSession, bool)> {
    let LocalBrainSession {
        config,
        providers,
        storage,
        brain_path,
        embedding,
        dimension,
    } = brain;

    let provider_config = embedding.to_provider_config(Some(dimension));
    let embed_provider = create_embed_provider(&provider_config)?;

    Ok((
        LocalEmbedSession {
            config,
            providers,
            storage,
            embed_provider,
            brain_path,
            embedding,
        },
        created,
    ))
}

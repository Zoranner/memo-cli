//! 打开本地 brain 时使用的向量维度（须与 Lance 表、metadata 一致）

use anyhow::Result;
use std::path::Path;

use crate::config::{AppConfig, ProvidersConfig};
use memo_local::DatabaseMetadata;

/// 优先使用 `metadata.json` 中的维度；尚无元数据时从 embedding 服务配置读取 `dimension`。
pub fn resolve_storage_dimension(
    brain_path: &Path,
    providers: &ProvidersConfig,
    config: &AppConfig,
) -> Result<usize> {
    if DatabaseMetadata::exists(brain_path) {
        return Ok(DatabaseMetadata::load(brain_path)?.dimension);
    }
    let embed = config.resolve_embedding(providers)?;
    embed.require_dimension()
}

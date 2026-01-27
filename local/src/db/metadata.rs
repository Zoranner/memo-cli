use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// 数据库元数据
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseMetadata {
    /// Embedding 模型名称
    pub model: String,
    /// 向量维度
    pub dimension: usize,
    /// 数据库创建时间
    pub created_at: DateTime<Utc>,
    /// 元数据格式版本
    pub version: String,
}

impl DatabaseMetadata {
    /// 创建新的元数据
    pub fn new(model: String, dimension: usize) -> Self {
        Self {
            model,
            dimension,
            created_at: Utc::now(),
            version: "1.0".to_string(),
        }
    }

    /// 从数据库目录加载元数据
    pub fn load(brain_path: &Path) -> Result<Self> {
        let metadata_path = brain_path.join("metadata.json");

        if !metadata_path.exists() {
            anyhow::bail!(
                "Database metadata not found. Please run 'memo init' first or the database may be from an older version."
            );
        }

        let content = std::fs::read_to_string(&metadata_path).with_context(|| {
            format!("Failed to read metadata file: {}", metadata_path.display())
        })?;

        let metadata: Self =
            serde_json::from_str(&content).with_context(|| "Failed to parse metadata file")?;

        Ok(metadata)
    }

    /// 保存元数据到数据库目录
    pub fn save(&self, brain_path: &Path) -> Result<()> {
        let metadata_path = brain_path.join("metadata.json");

        let content =
            serde_json::to_string_pretty(self).with_context(|| "Failed to serialize metadata")?;

        std::fs::write(&metadata_path, content).with_context(|| {
            format!("Failed to write metadata file: {}", metadata_path.display())
        })?;

        Ok(())
    }

    /// 验证维度是否匹配
    pub fn validate_dimension(&self, expected_dimension: usize) -> Result<()> {
        if self.dimension != expected_dimension {
            anyhow::bail!(
                "Vector dimension mismatch!\n\
                 Database dimension: {} (model: {})\n\
                 Current model dimension: {}\n\
                 \n\
                 The database was created with a different embedding model.\n\
                 To use a new model, you need to:\n\
                 1. Clear the database: memo clear --force\n\
                 2. Re-embed all your data\n\
                 \n\
                 Or switch back to the original model: {}",
                self.dimension,
                self.model,
                expected_dimension,
                self.model
            );
        }
        Ok(())
    }

    /// 检查元数据文件是否存在
    pub fn exists(brain_path: &Path) -> bool {
        brain_path.join("metadata.json").exists()
    }
}

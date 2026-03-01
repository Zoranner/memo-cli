use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use super::providers::{ProvidersConfig, ResolvedService};

/// 配置作用域
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigScope {
    Auto,
    Local,
    Global,
}

/// 应用配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    /// 数据库路径（可选，默认: ~/.memo/brain 或 ./.memo/brain）
    pub brain_path: Option<PathBuf>,

    /// Embedding 服务引用（如 "aliyun.embed"）
    pub embedding: String,

    /// Rerank 服务引用（如 "aliyun.rerank"）
    pub rerank: String,

    /// LLM 服务引用（可选，如 "aliyun.llm"）
    pub llm: Option<String>,

    /// 搜索结果数量上限（默认: 10）
    #[serde(default = "default_search_limit")]
    pub search_limit: usize,

    /// 第一层搜索阈值（0.0-1.0，默认: 0.35）
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

    /// 重复检测相似度阈值（0.0-1.0，默认: 0.85）
    #[serde(default = "default_duplicate_threshold")]
    pub duplicate_threshold: f32,
}

fn default_search_limit() -> usize {
    10
}

fn default_similarity_threshold() -> f32 {
    0.35
}

fn default_duplicate_threshold() -> f32 {
    0.85
}

impl AppConfig {
    /// 全局 .memo 目录：~/.memo/
    pub fn global_memo_dir() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".memo")
    }

    /// 本地 .memo 目录：./.memo/
    pub fn local_memo_dir() -> PathBuf {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(".memo")
    }

    /// 检查本地配置是否存在
    /// 注意：如果当前目录是用户主目录，则不认为是本地配置
    pub fn has_local_config() -> bool {
        // 获取当前目录
        let current_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(_) => return false,
        };

        // 获取全局 .memo 目录的父目录（用户主目录）
        let global_parent = Self::global_memo_dir().parent().map(|p| p.to_path_buf());

        // 如果当前目录就是用户主目录，不应该被当作本地配置
        if let Some(home) = global_parent {
            let current_canonical = current_dir.canonicalize().unwrap_or(current_dir.clone());
            let home_canonical = home.canonicalize().unwrap_or(home);

            if current_canonical == home_canonical {
                return false;
            }
        }

        // 检查本地配置文件是否存在
        Self::local_memo_dir().join("config.toml").exists()
    }

    /// 验证作用域标志（不能同时指定 local 和 global）
    pub fn validate_scope_flags(local: bool, global: bool) -> Result<()> {
        if local && global {
            anyhow::bail!("Cannot specify both --local and --global, please choose one");
        }
        Ok(())
    }

    /// 获取当前作用域名称
    /// 返回 "local" 或 "global"
    pub fn get_scope_name(force_local: bool, force_global: bool) -> &'static str {
        if force_local {
            "local"
        } else if force_global {
            "global"
        } else if Self::has_local_config() {
            "local"
        } else {
            "global"
        }
    }

    /// 根据 local 标志获取配置目录
    pub fn get_memo_dir(local: bool) -> PathBuf {
        if local {
            Self::local_memo_dir()
        } else {
            Self::global_memo_dir()
        }
    }

    /// 加载配置：根据 local/global 标志或优先级加载
    /// - local = true: 强制使用本地配置
    /// - global = true: 强制使用全局配置
    /// - 两者都为 false: 优先本地配置，其次全局配置
    pub fn load_with_scope(force_local: bool, force_global: bool) -> Result<Self> {
        Self::validate_scope_flags(force_local, force_global)?;

        let scope = if force_local {
            ConfigScope::Local
        } else if force_global {
            ConfigScope::Global
        } else {
            ConfigScope::Auto
        };

        Self::load_with_scope_internal(scope)
    }

    /// 加载配置：优先本地配置，其次全局配置
    pub fn load() -> Result<Self> {
        Self::load_with_scope_internal(ConfigScope::Auto)
    }

    /// 内部加载逻辑
    fn load_with_scope_internal(scope: ConfigScope) -> Result<Self> {
        match scope {
            ConfigScope::Auto => {
                // 优先本地配置
                if Self::has_local_config() {
                    Self::load_from_path(&Self::local_memo_dir().join("config.toml"), true)
                } else {
                    Self::load_from_path(&Self::global_memo_dir().join("config.toml"), false)
                }
            }
            ConfigScope::Local => {
                Self::load_from_path(&Self::local_memo_dir().join("config.toml"), true)
            }
            ConfigScope::Global => {
                Self::load_from_path(&Self::global_memo_dir().join("config.toml"), false)
            }
        }
    }

    /// 从指定路径加载配置文件
    fn load_from_path(path: &PathBuf, is_local: bool) -> Result<Self> {
        if !path.exists() {
            anyhow::bail!(
                "Configuration not found at: {}\nPlease create it from config.example.toml",
                path.display()
            );
        }

        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read config: {}", path.display()))?;

        let mut config: Self = toml::from_str(&content)
            .with_context(|| format!("Failed to parse config: {}", path.display()))?;

        // 本地配置强制使用本地 brain 路径
        if is_local {
            config.brain_path = Some(Self::local_memo_dir().join("brain"));
        }

        tracing::debug!("Loaded app config from: {}", path.display());
        tracing::debug!("Embedding: {}", config.embedding);
        tracing::debug!("Rerank: {}", config.rerank);

        Ok(config)
    }

    /// 获取数据库路径
    pub fn get_brain_path(&self) -> Result<PathBuf> {
        if let Some(path) = &self.brain_path {
            Ok(path.clone())
        } else {
            Ok(Self::global_memo_dir().join("brain"))
        }
    }

    /// 确保必要的目录存在
    pub fn ensure_dirs(&self) -> Result<()> {
        let brain_path = self.get_brain_path()?;
        std::fs::create_dir_all(&brain_path).with_context(|| {
            format!(
                "Failed to create database directory: {}",
                brain_path.display()
            )
        })?;
        Ok(())
    }

    /// 解析 embedding 服务配置
    pub fn resolve_embedding(&self, providers: &ProvidersConfig) -> Result<ResolvedService> {
        providers
            .get_service(&self.embedding)
            .with_context(|| format!("Failed to resolve embedding service: {}", self.embedding))
    }

    /// 解析 rerank 服务配置
    pub fn resolve_rerank(&self, providers: &ProvidersConfig) -> Result<ResolvedService> {
        providers
            .get_service(&self.rerank)
            .with_context(|| format!("Failed to resolve rerank service: {}", self.rerank))
    }

    /// 保存配置到全局目录
    #[allow(dead_code)]
    pub fn save(&self) -> Result<()> {
        let global_memo_dir = Self::global_memo_dir();
        std::fs::create_dir_all(&global_memo_dir).with_context(|| {
            format!(
                "Failed to create global memo directory: {}",
                global_memo_dir.display()
            )
        })?;

        let config_path = global_memo_dir.join("config.toml");
        let content = toml::to_string_pretty(self).with_context(|| "Failed to serialize config")?;

        std::fs::write(&config_path, content)
            .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_app_config() {
        let toml_str = r#"
embedding = "aliyun.embed"
rerank = "aliyun.rerank"
llm = "aliyun.llm"

search_limit = 10
similarity_threshold = 0.35
duplicate_threshold = 0.85
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.embedding, "aliyun.embed");
        assert_eq!(config.rerank, "aliyun.rerank");
        assert_eq!(config.llm, Some("aliyun.llm".to_string()));
        assert_eq!(config.search_limit, 10);
        assert_eq!(config.similarity_threshold, 0.35);
        assert_eq!(config.duplicate_threshold, 0.85);
    }

    #[test]
    fn test_default_values() {
        let toml_str = r#"
embedding = "aliyun.embed"
rerank = "aliyun.rerank"
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.search_limit, 10);
        assert_eq!(config.similarity_threshold, 0.35);
        assert_eq!(config.duplicate_threshold, 0.85);
    }
}

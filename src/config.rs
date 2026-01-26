use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// 默认值函数
fn default_brain_path() -> PathBuf {
    Config::global_memo_dir().join("brain")
}

fn default_embedding_api_key() -> String {
    String::new()
}

fn default_embedding_model() -> String {
    "text-embedding-3-small".to_string()
}

fn default_search_limit() -> usize {
    5
}

fn default_similarity_threshold() -> f32 {
    0.7
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "default_brain_path")]
    pub brain_path: PathBuf,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_cache_dir: Option<PathBuf>,

    // Embedding API 配置
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_base_url: Option<String>,
    #[serde(default = "default_embedding_api_key")]
    pub embedding_api_key: String,
    #[serde(default = "default_embedding_model")]
    pub embedding_model: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub embedding_dimension: Option<usize>,

    // 搜索配置
    #[serde(default = "default_search_limit")]
    pub search_limit: usize,
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,
}

impl Default for Config {
    fn default() -> Self {
        let global_memo_dir = Self::global_memo_dir();

        Self {
            brain_path: global_memo_dir.join("brain"),
            model_cache_dir: None,

            // 默认使用 OpenAI API (需要用户配置 API key)
            embedding_provider: None,
            embedding_base_url: None,
            embedding_api_key: String::new(),
            embedding_model: "text-embedding-3-small".to_string(),
            embedding_dimension: None,

            search_limit: 5,
            similarity_threshold: 0.7,
        }
    }
}

impl Config {
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
            // 使用 canonicalize 解析符号链接，但如果失败就直接比较
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
    /// - 两者都为 false: 优先本地配置，其次全局配置，最后默认配置
    pub fn load_with_scope(force_local: bool, force_global: bool) -> Result<Self> {
        Self::validate_scope_flags(force_local, force_global)?;

        if force_local {
            // 强制使用本地配置
            return Self::load_from_path(&Self::local_memo_dir().join("config.toml"), true);
        }

        if force_global {
            // 强制使用全局配置
            return Self::load_from_path(&Self::global_memo_dir().join("config.toml"), false);
        }

        // 默认优先级：本地 > 全局 > 默认
        Self::load()
    }

    /// 从指定路径加载配置文件
    fn load_from_path(path: &std::path::Path, is_local: bool) -> Result<Self> {
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read config file: {}", path.display()))?;
            let mut config: Config = toml::from_str(&content)
                .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

            // 本地配置需要覆盖数据库路径
            if is_local {
                config.brain_path = Self::local_memo_dir().join("brain");
            }

            Ok(config)
        } else {
            // 配置文件不存在，使用默认配置
            if is_local {
                Ok(Self {
                    brain_path: Self::local_memo_dir().join("brain"),
                    ..Self::default()
                })
            } else {
                Ok(Self::default())
            }
        }
    }

    /// 加载配置：优先本地配置，其次全局配置，最后默认配置
    pub fn load() -> Result<Self> {
        // 1. 尝试本地配置（排除用户主目录）
        let local_config_path = Self::local_memo_dir().join("config.toml");
        if Self::has_local_config() {
            let content = std::fs::read_to_string(&local_config_path).with_context(|| {
                format!(
                    "Failed to read local config file: {}",
                    local_config_path.display()
                )
            })?;
            let mut config: Config =
                toml::from_str(&content).with_context(|| "Failed to parse local config file")?;

            // 使用本地数据库路径
            config.brain_path = Self::local_memo_dir().join("brain");

            return Ok(config);
        }

        // 2. 尝试全局配置
        let global_config_path = Self::global_memo_dir().join("config.toml");
        if global_config_path.exists() {
            let content = std::fs::read_to_string(&global_config_path).with_context(|| {
                format!(
                    "Failed to read global config file: {}",
                    global_config_path.display()
                )
            })?;
            let config: Config =
                toml::from_str(&content).with_context(|| "Failed to parse global config file")?;

            return Ok(config);
        }

        // 3. 使用默认配置
        Ok(Self::default())
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

    /// 确保必要的目录存在
    pub fn ensure_dirs(&self) -> Result<()> {
        // 数据库目录
        std::fs::create_dir_all(&self.brain_path).with_context(|| {
            format!(
                "Failed to create database directory: {}",
                self.brain_path.display()
            )
        })?;

        Ok(())
    }
}

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

// ============================================
// 嵌入配置
// ============================================

/// 嵌入配置（记录记忆时）
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct EmbedConfig {
    /// Embedding 服务引用
    pub embedding_provider: String,

    /// 重复检测阈值（0.0-1.0，默认: 0.85）
    #[serde(default = "default_duplicate_threshold")]
    pub duplicate_threshold: f32,
}

impl Default for EmbedConfig {
    fn default() -> Self {
        Self {
            embedding_provider: String::new(),
            duplicate_threshold: default_duplicate_threshold(),
        }
    }
}

fn default_duplicate_threshold() -> f32 {
    0.85
}

// ============================================
// 搜索配置
// ============================================

/// 搜索配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct SearchConfig {
    /// Rerank 服务引用
    pub rerank_provider: String,

    /// LLM 服务引用（默认用于拆解和总结）
    pub llm_provider: String,

    /// 返回结果数量上限（默认: 10）
    #[serde(default = "default_results_limit")]
    pub results_limit: usize,

    /// 向量搜索相似度阈值（0.0-1.0，默认: 0.35）
    #[serde(default = "default_similarity_threshold")]
    pub similarity_threshold: f32,

    /// 多层搜索深度（默认: 5，设为 1 禁用多层扩展）
    #[serde(default = "default_max_depth")]
    pub max_depth: usize,

    /// 每层扩展分支数（默认: 5）
    #[serde(default = "default_branch_limit")]
    pub branch_limit: usize,

    /// 扩展时是否要求标签重叠（默认: true）
    #[serde(default = "default_require_tag_overlap")]
    pub require_tag_overlap: bool,
}

impl Default for SearchConfig {
    fn default() -> Self {
        Self {
            rerank_provider: String::new(),
            llm_provider: String::new(),
            results_limit: default_results_limit(),
            similarity_threshold: default_similarity_threshold(),
            max_depth: default_max_depth(),
            branch_limit: default_branch_limit(),
            require_tag_overlap: default_require_tag_overlap(),
        }
    }
}

fn default_results_limit() -> usize {
    10
}

fn default_similarity_threshold() -> f32 {
    0.35
}

fn default_max_depth() -> usize {
    5
}

fn default_branch_limit() -> usize {
    5
}

fn default_require_tag_overlap() -> bool {
    true
}

// ============================================
// 搜索 > 拆解配置
// ============================================

/// 拆解配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DecomposeConfig {
    /// LLM 服务引用（可选，覆盖 search.llm_provider）
    pub llm_provider: Option<String>,

    /// 最大叶子数（默认: 12）
    #[serde(default = "default_max_leaves")]
    pub max_leaves: usize,

    /// 拆解策略提示词（可选，覆盖内置策略）
    pub strategy_prompt: Option<String>,
}

impl Default for DecomposeConfig {
    fn default() -> Self {
        Self {
            llm_provider: None,
            max_leaves: default_max_leaves(),
            strategy_prompt: None,
        }
    }
}

fn default_max_leaves() -> usize {
    12
}

// ============================================
// 搜索 > 合并配置
// ============================================

/// 合并配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct MergeConfig {
    /// 每个查询的候选数（默认: 50）
    #[serde(default = "default_candidates_per_query")]
    pub candidates_per_query: usize,

    /// 每个叶子的结果数（默认: 5）
    #[serde(default = "default_results_per_leaf")]
    pub results_per_leaf: usize,

    /// 最大结果数（默认: 20）
    #[serde(default = "default_max_results")]
    pub max_results: usize,

    /// 去重阈值（0.0-1.0，默认: 0.98）
    #[serde(default = "default_dedup_threshold")]
    pub dedup_threshold: f32,
}

impl Default for MergeConfig {
    fn default() -> Self {
        Self {
            candidates_per_query: default_candidates_per_query(),
            results_per_leaf: default_results_per_leaf(),
            max_results: default_max_results(),
            dedup_threshold: default_dedup_threshold(),
        }
    }
}

fn default_candidates_per_query() -> usize {
    50
}

fn default_results_per_leaf() -> usize {
    5
}

fn default_max_results() -> usize {
    20
}

fn default_dedup_threshold() -> f32 {
    0.98
}

// ============================================
// 搜索 > 总结配置
// ============================================

/// 总结配置
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SummarizeConfig {
    /// LLM 服务引用（可选，覆盖 search.llm_provider）
    pub llm_provider: Option<String>,

    /// 总结策略提示词（可选，覆盖内置策略）
    pub strategy_prompt: Option<String>,
}

// ============================================
// 应用配置
// ============================================

/// 应用配置
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AppConfig {
    /// 数据库路径（可选，默认: ~/.memo/brain 或 ./.memo/brain）
    pub brain_path: Option<PathBuf>,

    /// 嵌入配置
    #[serde(default)]
    pub embed: EmbedConfig,

    /// 搜索配置
    #[serde(default)]
    pub search: SearchConfig,

    /// 拆解配置
    #[serde(default)]
    pub decompose: DecomposeConfig,

    /// 合并配置
    #[serde(default)]
    pub merge: MergeConfig,

    /// 总结配置
    #[serde(default)]
    pub summarize: SummarizeConfig,
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
        let current_dir = match std::env::current_dir() {
            Ok(dir) => dir,
            Err(_) => return false,
        };

        let global_parent = Self::global_memo_dir().parent().map(|p| p.to_path_buf());

        if let Some(home) = global_parent {
            let current_canonical = current_dir.canonicalize().unwrap_or(current_dir.clone());
            let home_canonical = home.canonicalize().unwrap_or(home);

            if current_canonical == home_canonical {
                return false;
            }
        }

        Self::local_memo_dir().join("config.toml").exists()
    }

    /// 验证作用域标志
    pub fn validate_scope_flags(local: bool, global: bool) -> Result<()> {
        if local && global {
            anyhow::bail!("Cannot specify both --local and --global, please choose one");
        }
        Ok(())
    }

    /// 获取当前作用域名称
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

    /// 加载配置
    pub fn load_with_scope(force_local: bool, force_global: bool) -> Result<Self> {
        Self::validate_scope_flags(force_local, force_global)?;

        if !force_local && !force_global {
            return Self::load();
        }

        let scope = if force_local {
            ConfigScope::Local
        } else {
            ConfigScope::Global
        };

        Self::load_with_scope_internal(scope)
    }

    /// 加载配置：优先本地配置
    pub fn load() -> Result<Self> {
        Self::load_with_scope_internal(ConfigScope::Auto)
    }

    /// 内部加载逻辑
    fn load_with_scope_internal(scope: ConfigScope) -> Result<Self> {
        match scope {
            ConfigScope::Auto => {
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

        if is_local {
            config.brain_path = Some(Self::local_memo_dir().join("brain"));
        }

        tracing::debug!("Loaded app config from: {}", path.display());
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
            .get_service(&self.embed.embedding_provider)
            .with_context(|| {
                format!(
                    "Failed to resolve embedding service: {}",
                    self.embed.embedding_provider
                )
            })
    }

    /// 解析 rerank 服务配置
    pub fn resolve_rerank(&self, providers: &ProvidersConfig) -> Result<ResolvedService> {
        providers
            .get_service(&self.search.rerank_provider)
            .with_context(|| {
                format!(
                    "Failed to resolve rerank service: {}",
                    self.search.rerank_provider
                )
            })
    }

    /// 解析拆解用的 LLM 服务配置
    pub fn resolve_decompose_llm(&self, providers: &ProvidersConfig) -> Result<ResolvedService> {
        let llm_provider = self
            .decompose
            .llm_provider
            .as_ref()
            .unwrap_or(&self.search.llm_provider);
        providers
            .get_service(llm_provider)
            .with_context(|| format!("Failed to resolve decompose LLM service: {}", llm_provider))
    }

    /// 解析总结用的 LLM 服务配置
    pub fn resolve_summarize_llm(&self, providers: &ProvidersConfig) -> Result<ResolvedService> {
        let llm_provider = self
            .summarize
            .llm_provider
            .as_ref()
            .unwrap_or(&self.search.llm_provider);
        providers
            .get_service(llm_provider)
            .with_context(|| format!("Failed to resolve summarize LLM service: {}", llm_provider))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_app_config() {
        let toml_str = r#"
[embed]
embedding_provider = "aliyun.embed"
duplicate_threshold = 0.85

[search]
rerank_provider = "aliyun.rerank"
llm_provider = "aliyun.llm"
results_limit = 10
similarity_threshold = 0.35
max_depth = 5
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.embed.embedding_provider, "aliyun.embed");
        assert_eq!(config.embed.duplicate_threshold, 0.85);
        assert_eq!(config.search.rerank_provider, "aliyun.rerank");
        assert_eq!(config.search.llm_provider, "aliyun.llm");
        assert_eq!(config.search.results_limit, 10);
        assert_eq!(config.search.similarity_threshold, 0.35);
        assert_eq!(config.search.max_depth, 5);
    }

    #[test]
    fn test_default_values() {
        let toml_str = r#"
[embed]
embedding_provider = "aliyun.embed"

[search]
rerank_provider = "aliyun.rerank"
llm_provider = "aliyun.llm"
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.search.results_limit, 10);
        assert_eq!(config.search.similarity_threshold, 0.35);
        assert_eq!(config.embed.duplicate_threshold, 0.85);
        assert_eq!(config.decompose.max_leaves, 12);
        assert_eq!(config.merge.max_results, 20);
    }

    #[test]
    fn test_decompose_config() {
        let toml_str = r#"
[embed]
embedding_provider = "aliyun.embed"

[search]
rerank_provider = "aliyun.rerank"
llm_provider = "aliyun.llm"

[decompose]
max_leaves = 8
strategy_prompt = "按三维模型拆解"

[merge]
results_per_leaf = 3
        "#;

        let config: AppConfig = toml::from_str(toml_str).unwrap();

        assert_eq!(config.decompose.max_leaves, 8);
        assert_eq!(
            config.decompose.strategy_prompt,
            Some("按三维模型拆解".to_string())
        );
        assert_eq!(config.merge.results_per_leaf, 3);
    }
}

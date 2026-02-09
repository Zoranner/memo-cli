use anyhow::Result;

use crate::config::{AppConfig, ProvidersConfig};
use crate::providers::create_embed_provider;
use crate::ui::Output;
use memo_local::{DatabaseMetadata, LocalStorageClient};
use memo_types::{StorageBackend, StorageConfig};

// === 公开接口 ===

/// 显式初始化（带用户反馈）
/// local: true 表示在本地目录初始化，false 表示在全局目录初始化
pub async fn initialize(local: bool) -> Result<()> {
    let output = Output::new();
    let config_dir = AppConfig::get_memo_dir(local);
    let config_path = config_dir.join("config.toml");
    let providers_path = AppConfig::global_memo_dir().join("providers.toml");
    let location = AppConfig::get_scope_name(local, false);

    // 创建配置目录
    std::fs::create_dir_all(&config_dir)?;

    // 检查 providers.toml（只在全局目录）
    if !providers_path.exists() {
        output.resource_action("Creating", "providers config template", &providers_path);
        // 注意：这里只创建示例，用户需要手动填写 API keys
        output.note("Please create providers.toml from providers.example.toml");
        output.info(&format!(
            "Example: cp providers.example.toml {}",
            providers_path.display()
        ));
    }

    // 如果配置文件已存在，提示用户
    if config_path.exists() {
        output.resource_action("Found", "config", &config_path);
    } else {
        // 创建配置文件
        let config_content = r#"# Memo 主配置文件
# 服务配置（引用 providers.toml 中的服务）
embedding = "aliyun.embed"
rerank = "aliyun.rerank"

# 搜索配置
search_limit = 10
similarity_threshold = 0.35
duplicate_threshold = 0.85
"#;

        std::fs::write(&config_path, config_content)?;
        output.resource_action("Creating", "config", &config_path);

        // 提示用户配置
        output.note("Please edit the config file to set your preferred services");
        output.info(&format!("Config file: {}", config_path.display()));
    }

    // 加载配置并确保目录存在
    let config = AppConfig::load_with_scope(local, !local)?;
    config.ensure_dirs()?;

    // 加载 providers 并创建 embedding provider
    let providers = ProvidersConfig::load()?;
    let embed_config = config.resolve_embedding(&providers)?;
    let embed_provider = create_embed_provider(&embed_config)?;

    // 确保 memories 表存在
    let brain_path = config.get_brain_path()?;
    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension: embed_provider.dimension(),
    };

    let storage = LocalStorageClient::connect(&storage_config).await?;
    let table_path = brain_path.join("memories.lance");
    let metadata_path = brain_path.join("metadata.json");

    if !storage.exists().await? {
        storage.init().await?;
        output.resource_action("Creating", "database", &table_path);

        // 创建元数据
        let metadata =
            DatabaseMetadata::new(embed_config.model.clone(), embed_provider.dimension());
        metadata.save(&brain_path)?;
        output.resource_action("Creating", "metadata", &metadata_path);
    } else {
        output.resource_action("Found", "database", &table_path);
        output.resource_action("Found", "metadata", &metadata_path);
    }

    output.finish("initialization", location);

    Ok(())
}

// === 辅助函数 ===

/// 自动初始化（静默模式）
/// 确保数据库目录和表存在，不生成配置文件
/// 返回是否进行了初始化
pub async fn ensure_initialized() -> Result<bool> {
    // 加载配置
    let config = AppConfig::load()?;
    let mut initialized = false;

    // 确保必要的目录存在
    config.ensure_dirs()?;

    // 加载 providers 并创建 embedding provider
    let providers = ProvidersConfig::load()?;
    let embed_config = config.resolve_embedding(&providers)?;
    let embed_provider = create_embed_provider(&embed_config)?;

    // 确保 memories 表存在
    let brain_path = config.get_brain_path()?;
    let storage_config = StorageConfig {
        path: brain_path.to_string_lossy().to_string(),
        dimension: embed_provider.dimension(),
    };

    let storage = LocalStorageClient::connect(&storage_config).await?;
    if !storage.exists().await? {
        storage.init().await?;

        // 创建元数据
        let metadata =
            DatabaseMetadata::new(embed_config.model.clone(), embed_provider.dimension());
        metadata.save(&brain_path)?;

        initialized = true;
    }

    Ok(initialized)
}

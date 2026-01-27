use anyhow::Result;

use crate::config::Config;
use crate::embedding::EmbeddingModel;
use crate::ui::Output;
use memo_local::{DatabaseMetadata, LocalStorageClient};
use memo_types::{StorageBackend, StorageConfig};

/// 显式初始化（带用户反馈）
/// local: true 表示在本地目录初始化，false 表示在全局目录初始化
pub async fn initialize(local: bool) -> Result<()> {
    let output = Output::new();
    let config_dir = Config::get_memo_dir(local);
    let config_path = config_dir.join("config.toml");
    let location = Config::get_scope_name(local, false);

    // 创建配置目录
    std::fs::create_dir_all(&config_dir)?;

    // 如果配置文件已存在，提示用户
    if config_path.exists() {
        output.resource_action("Found", "config", &config_path);
    } else {
        // 创建配置文件
        let mut config = Config::default();

        // 如果是本地初始化，设置本地数据库路径
        if local {
            config.brain_path = Config::local_memo_dir().join("brain");
        }

        // 保存配置
        let content = toml::to_string_pretty(&config)?;
        std::fs::write(&config_path, content)?;

        output.resource_action("Creating", "config", &config_path);

        // 提示用户配置 API key
        output.note("Please edit the config file and set your embedding_api_key");
        output.info(&format!("Config file: {}", config_path.display()));
        output.info("Supports OpenAI, Azure OpenAI, Jina AI and all OpenAI-compatible APIs");
    }

    // 加载配置并确保目录存在
    let config = if local {
        let mut config = Config::load()?;
        config.brain_path = Config::local_memo_dir().join("brain");
        config
    } else {
        Config::load()?
    };

    config.ensure_dirs()?;

    // 创建 embedding 模型以获取维度信息
    let model = EmbeddingModel::new(
        config.embedding_api_key.clone(),
        config.embedding_model.clone(),
        config.embedding_base_url.clone(),
        config.embedding_dimension,
        config.embedding_provider.clone(),
    )?;

    // 确保 memories 表存在
    let storage_config = StorageConfig {
        path: config.brain_path.to_string_lossy().to_string(),
        dimension: model.dimension(),
    };

    let storage = LocalStorageClient::connect(&storage_config).await?;
    let table_path = config.brain_path.join("memories.lance");
    let metadata_path = config.brain_path.join("metadata.json");

    if !storage.exists().await? {
        storage.init().await?;
        output.resource_action("Creating", "database", &table_path);

        // 创建元数据
        let metadata = DatabaseMetadata::new(config.embedding_model.clone(), model.dimension());
        metadata.save(&config.brain_path)?;
        output.resource_action("Creating", "metadata", &metadata_path);
    } else {
        output.resource_action("Found", "database", &table_path);
        output.resource_action("Found", "metadata", &metadata_path);
    }

    output.finish("initialization", location);

    Ok(())
}

/// 自动初始化（静默模式）
/// 确保数据库目录和表存在，不生成配置文件
/// 返回是否进行了初始化
pub async fn ensure_initialized() -> Result<bool> {
    let config = Config::load()?;
    let mut initialized = false;

    // 确保必要的目录存在
    config.ensure_dirs()?;

    // 创建 embedding 模型以获取维度信息
    let model = EmbeddingModel::new(
        config.embedding_api_key.clone(),
        config.embedding_model.clone(),
        config.embedding_base_url.clone(),
        config.embedding_dimension,
        config.embedding_provider.clone(),
    )?;

    // 确保 memories 表存在
    let storage_config = StorageConfig {
        path: config.brain_path.to_string_lossy().to_string(),
        dimension: model.dimension(),
    };

    let storage = LocalStorageClient::connect(&storage_config).await?;
    if !storage.exists().await? {
        storage.init().await?;

        // 创建元数据
        let metadata = DatabaseMetadata::new(config.embedding_model.clone(), model.dimension());
        metadata.save(&config.brain_path)?;

        initialized = true;
    }

    Ok(initialized)
}

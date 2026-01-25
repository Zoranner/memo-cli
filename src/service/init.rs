use anyhow::Result;

use crate::config::Config;
use crate::db::{Connection, TableOperations};
use crate::output::Output;

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
        eprintln!();
        output.note("⚠️  请编辑配置文件,设置你的 embedding_api_key");
        eprintln!("{:>12} 配置文件位置: {}", "", config_path.display());
        eprintln!("{:>12} 支持 OpenAI、Azure OpenAI、Jina AI 等所有 OpenAI 兼容 API", "");
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

    // 确保 memories 表存在
    let conn = Connection::connect(config.brain_path.to_str().unwrap()).await?;
    let db_path = config.brain_path.join("memories.lance");
    if !TableOperations::table_exists(conn.inner(), "memories").await {
        TableOperations::create_table(conn.inner(), "memories").await?;
        output.resource_action("Creating", "database", &db_path);
    } else {
        output.resource_action("Found", "database", &db_path);
    }

    eprintln!();
    output.finish("initialization", location);

    Ok(())
}

/// 自动初始化（带提示）
/// 确保数据库目录和配置文件存在
/// 返回是否进行了初始化
pub async fn ensure_initialized() -> Result<bool> {
    let output = Output::new();
    let config = Config::load()?;
    let mut initialized = false;

    // 如果全局配置文件不存在，保存默认配置
    let global_config_path = Config::global_memo_dir().join("config.toml");
    if !global_config_path.exists() {
        config.ensure_dirs()?;
        config.save()?;
        output.resource_action("Creating", "config", &global_config_path);
        initialized = true;
    } else {
        config.ensure_dirs()?;
    }

    // 确保 memories 表存在
    let conn = Connection::connect(config.brain_path.to_str().unwrap()).await?;
    let db_path = config.brain_path.join("memories.lance");
    if !TableOperations::table_exists(conn.inner(), "memories").await {
        TableOperations::create_table(conn.inner(), "memories").await?;
        output.resource_action("Creating", "database", &db_path);
        initialized = true;
    }

    Ok(initialized)
}

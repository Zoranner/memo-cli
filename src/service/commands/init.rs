use anyhow::Result;

use crate::config::AppConfig;
use crate::service::session::open_local_embed_session;
use crate::ui::Output;

/// 显式初始化（带用户反馈）
pub async fn initialize(local: bool) -> Result<()> {
    let output = Output::new();
    let config_dir = AppConfig::get_memo_dir(local);
    let config_path = config_dir.join("config.toml");
    let providers_path = AppConfig::global_memo_dir().join("providers.toml");
    let location = AppConfig::get_scope_name(local, false);

    std::fs::create_dir_all(&config_dir)?;

    if !providers_path.exists() {
        output.resource_action("Creating", "providers config template", &providers_path);
        output.note("Please create providers.toml from providers.example.toml");
        output.info(&format!(
            "Example: cp providers.example.toml {}",
            providers_path.display()
        ));
    }

    if config_path.exists() {
        output.resource_action("Found", "config", &config_path);
    } else {
        let config_content = r#"# Memo 主配置文件
# 服务配置（引用 providers.toml 中的服务）
llm = "aliyun.llm"
embedding = "aliyun.embed"
rerank = "aliyun.rerank"

# 搜索配置
search_limit = 10
similarity_threshold = 0.35
duplicate_threshold = 0.85
"#;

        std::fs::write(&config_path, config_content)?;
        output.resource_action("Creating", "config", &config_path);

        output.note("Please edit the config file to set your preferred services");
        output.info(&format!("Config file: {}", config_path.display()));
    }

    let (session, created) = open_local_embed_session(local, !local).await?;
    let brain_path = session.brain_path;
    let table_path = brain_path.join("memories.lance");
    let metadata_path = brain_path.join("metadata.json");

    if created {
        output.resource_action("Creating", "database", &table_path);
        output.resource_action("Creating", "metadata", &metadata_path);
    } else {
        output.resource_action("Found", "database", &table_path);
        output.resource_action("Found", "metadata", &metadata_path);
    }

    output.finish("initialization", location);

    Ok(())
}

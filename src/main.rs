use anyhow::Result;
use clap::Parser;

// Core modules
mod cli;
mod config;

// UI module
mod ui;

// Infrastructure
mod llm;
mod parser;

// Business logic
mod service;

fn main() {
    // 初始化日志（从环境变量 RUST_LOG 读取日志级别，默认为 warn）
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let args = cli::Cli::parse();

    // 统一创建 Tokio Runtime，避免重复创建
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");

    let result = runtime.block_on(async {
        match args.command {
            // 初始化
            cli::Commands::Init { local } => service::init::initialize(local).await,

            // 核心操作
            cli::Commands::Embed {
                input,
                tags,
                force,
                dup_threshold,
                local,
                global,
            } => service::embed::embed(input, tags, force, dup_threshold, local, global).await,
            cli::Commands::Search {
                query,
                limit,
                threshold,
                after,
                before,
                local,
                global,
            } => {
                service::search::search(service::search::SearchOptions {
                    query,
                    limit,
                    threshold,
                    after,
                    before,
                    force_local: local,
                    force_global: global,
                })
                .await
            }
            cli::Commands::List { local, global } => service::list::list(local, global).await,

            // 记忆管理
            cli::Commands::Update {
                id,
                content,
                tags,
                local,
                global,
            } => service::update::update(&id, content, tags, local, global).await,
            cli::Commands::Merge {
                ids,
                content,
                tags,
                local,
                global,
            } => service::merge::merge(ids, content, tags, local, global).await,
            cli::Commands::Delete {
                id,
                local,
                global,
                force,
            } => service::delete::delete(&id, local, global, force).await,
            cli::Commands::Clear {
                local,
                global,
                force,
            } => service::clear::clear(local, global, force).await,
        }
    });

    if let Err(e) = result {
        let output = ui::Output::new();
        output.error(&format!("{:#}", e));
        std::process::exit(1);
    }
}

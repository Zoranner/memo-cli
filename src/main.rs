use anyhow::Result;
use clap::Parser;

// Core modules
mod cli;
mod config;
mod models;
mod output;

// Infrastructure
mod db;
mod embedding;
mod parser;

// Business logic
mod service;

fn main() -> Result<()> {
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
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        match args.command {
            cli::Commands::Init { local } => service::init::initialize(local).await,
            cli::Commands::Embed {
                input,
                tags,
                local,
                global,
            } => service::embed::embed(input, tags, local, global).await,
            cli::Commands::Search {
                query,
                limit,
                threshold,
                after,
                before,
                local,
                global,
            } => {
                service::search::search(&query, limit, threshold, after, before, local, global)
                    .await
            }
            cli::Commands::List { local, global } => service::list::list(local, global).await,
            cli::Commands::Clear {
                local,
                global,
                force,
            } => service::clear::clear(local, global, force).await,
        }
    })
}

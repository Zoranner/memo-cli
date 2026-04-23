use anyhow::Result;
use clap::Parser;

mod cli;
mod config;
mod providers;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    cli::commands::run(cli::args::Cli::parse())
}

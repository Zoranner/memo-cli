use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use clap::{Parser, Subcommand};
use memo_engine::{
    ConsolidationTrigger, EntityInput, EpisodeInput, ExtractionSource, FactInput, MemoryEngine,
    RebuildScope, RetrieveRequest,
};

mod app_config;
mod lmkit_adapter;
mod lmkit_extraction_adapter;

#[derive(Parser)]
#[command(name = "memo")]
#[command(about = "Local single-process memory engine")]
struct Cli {
    #[arg(long, default_value = ".memo")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Init,
    Ingest {
        content: String,
        #[arg(long, default_value = "L1")]
        layer: String,
        #[arg(long = "entity")]
        entities: Vec<String>,
        #[arg(long = "fact")]
        facts: Vec<String>,
    },
    Query {
        query: String,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        deep: bool,
    },
    Inspect {
        id: String,
    },
    Dream {
        #[arg(long, default_value = "manual")]
        trigger: String,
    },
    RebuildIndex {
        #[arg(long, default_value = "all")]
        scope: String,
    },
    Stats,
    Benchmark {
        query: String,
        #[arg(long, default_value_t = 20)]
        iterations: usize,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
    },
}

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .with_target(false)
        .init();

    let Cli { data_dir, command } = Cli::parse();

    match command {
        Command::Init => {
            let report = app_config::initialize_data_dir(&data_dir)?;
            println!(
                "{}",
                serde_json::json!({
                    "data_dir": data_dir,
                    "config_created": report.config_created,
                    "providers_created": report.providers_created,
                })
            );
        }
        command => {
            let config = app_config::build_engine_config(&data_dir)?;
            let engine = MemoryEngine::open(config)?;

            match command {
                Command::Init => unreachable!("handled above"),
                Command::Ingest {
                    content,
                    layer,
                    entities,
                    facts,
                } => {
                    let input = EpisodeInput {
                        content,
                        layer: layer.parse()?,
                        entities: parse_entities(&entities)?,
                        facts: parse_facts(&facts)?,
                        source_episode_id: None,
                        confidence: 0.85,
                    };
                    let id = engine.ingest_episode(input)?;
                    println!("{}", id);
                }
                Command::Query { query, limit, deep } => {
                    let result = engine.query(RetrieveRequest { query, limit, deep })?;
                    println!("{}", serde_json::to_string_pretty(&result)?);
                }
                Command::Inspect { id } => {
                    let record = engine.inspect_memory(&id)?;
                    println!("{}", serde_json::to_string_pretty(&record)?);
                }
                Command::Dream { trigger } => {
                    let trigger = parse_trigger(&trigger)?;
                    let report = engine.consolidate(trigger)?;
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                Command::RebuildIndex { scope } => {
                    let scope = parse_scope(&scope)?;
                    let report = engine.rebuild_indexes(scope)?;
                    println!("{}", serde_json::to_string_pretty(&report)?);
                }
                Command::Stats => {
                    let stats = engine.stats()?;
                    println!("{}", serde_json::to_string_pretty(&stats)?);
                }
                Command::Benchmark {
                    query,
                    iterations,
                    limit,
                } => {
                    let mut elapsed_ms = 0_u128;
                    for _ in 0..iterations {
                        let started = Instant::now();
                        let _ = engine.query(RetrieveRequest {
                            query: query.clone(),
                            limit,
                            deep: false,
                        })?;
                        elapsed_ms += started.elapsed().as_millis();
                    }
                    let avg = elapsed_ms as f64 / iterations.max(1) as f64;
                    println!(
                        "{}",
                        serde_json::json!({
                            "iterations": iterations,
                            "avg_ms": avg,
                            "total_ms": elapsed_ms,
                        })
                    );
                }
            }
        }
    }

    Ok(())
}

fn parse_entities(raw: &[String]) -> Result<Vec<EntityInput>> {
    raw.iter()
        .map(|item| {
            let mut parts = item.splitn(3, ':');
            let entity_type = parts
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("invalid entity format: {}", item))?;
            let name = parts
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("invalid entity format: {}", item))?;
            let aliases = parts
                .next()
                .map(|value| {
                    value
                        .split('|')
                        .filter(|alias| !alias.is_empty())
                        .map(str::to_string)
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Ok(EntityInput {
                entity_type: entity_type.to_string(),
                name: name.to_string(),
                aliases,
                confidence: 0.9,
                source: ExtractionSource::Manual,
            })
        })
        .collect()
}

fn parse_facts(raw: &[String]) -> Result<Vec<FactInput>> {
    raw.iter()
        .map(|item| {
            let mut parts = item.splitn(3, ':');
            let subject = parts
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("invalid fact format: {}", item))?;
            let predicate = parts
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("invalid fact format: {}", item))?;
            let object = parts
                .next()
                .filter(|value| !value.is_empty())
                .ok_or_else(|| anyhow::anyhow!("invalid fact format: {}", item))?;
            Ok(FactInput {
                subject: subject.to_string(),
                predicate: predicate.to_string(),
                object: object.to_string(),
                confidence: 0.9,
                source: ExtractionSource::Manual,
            })
        })
        .collect()
}

fn parse_trigger(raw: &str) -> Result<ConsolidationTrigger> {
    Ok(match raw.to_ascii_lowercase().as_str() {
        "session_end" => ConsolidationTrigger::SessionEnd,
        "idle" => ConsolidationTrigger::Idle,
        "before_compaction" => ConsolidationTrigger::BeforeCompaction,
        "manual" => ConsolidationTrigger::Manual,
        _ => anyhow::bail!("invalid trigger: {}", raw),
    })
}

fn parse_scope(raw: &str) -> Result<RebuildScope> {
    Ok(match raw.to_ascii_lowercase().as_str() {
        "all" => RebuildScope::All,
        "text" => RebuildScope::Text,
        "vector" => RebuildScope::Vector,
        _ => anyhow::bail!("invalid rebuild scope: {}", raw),
    })
}

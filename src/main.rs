use std::{path::PathBuf, time::Instant};

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use memo_engine::{
    ConsolidationTrigger, EntityInput, EpisodeInput, ExtractionResult, ExtractionSource, FactInput,
    IngestPreview, MemoryEngine, RebuildScope, RetrieveRequest,
};

mod app_config;
mod lmkit_adapter;
mod lmkit_extraction_adapter;
mod lmkit_rerank_adapter;

#[derive(Parser)]
#[command(name = "memo")]
#[command(about = "Local single-process memory engine")]
struct Cli {
    #[arg(long, default_value = ".memo")]
    data_dir: PathBuf,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init,
    Extract {
        content: String,
    },
    Ingest {
        content: String,
        #[arg(long, default_value = "L1")]
        layer: String,
        #[arg(long)]
        session: Option<String>,
        #[arg(long)]
        at: Option<String>,
        #[arg(long = "entity")]
        entities: Vec<String>,
        #[arg(long = "fact")]
        facts: Vec<String>,
        #[arg(long)]
        dry_run: bool,
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
        #[arg(long)]
        enqueue: bool,
    },
    RunDreamJobs {
        #[arg(long, default_value_t = 1)]
        limit: usize,
    },
    RefreshIndex {
        #[arg(long, default_value = "all")]
        scope: String,
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
        Command::Extract { content } => {
            let provider = app_config::build_extraction_provider(&data_dir)?.ok_or_else(|| {
                anyhow::anyhow!("no extraction provider configured in config.toml")
            })?;
            let result = provider.extract(&content)?;
            println!("{}", render_extraction_result(&result)?);
        }
        command => {
            let config = app_config::build_engine_config(&data_dir)?;
            let engine = MemoryEngine::open(config)?;

            match command {
                Command::Init => unreachable!("handled above"),
                Command::Extract { .. } => unreachable!("handled above"),
                Command::Ingest {
                    content,
                    layer,
                    session,
                    at,
                    entities,
                    facts,
                    dry_run,
                } => {
                    let input = EpisodeInput {
                        content,
                        layer: layer.parse()?,
                        entities: parse_entities(&entities)?,
                        facts: parse_facts(&facts)?,
                        source_episode_id: None,
                        session_id: session,
                        recorded_at: parse_recorded_at(at.as_deref())?,
                        confidence: 0.85,
                    };
                    if dry_run {
                        let preview = engine.preview_ingest(&input)?;
                        println!("{}", render_ingest_preview(&preview)?);
                        return Ok(());
                    }
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
                Command::Dream { trigger, enqueue } => {
                    let trigger = parse_trigger(&trigger)?;
                    if enqueue {
                        let job_id = engine.schedule_consolidation(trigger)?;
                        println!(
                            "{}",
                            serde_json::to_string_pretty(&serde_json::json!({
                                "job_id": job_id,
                                "status": "pending",
                                "trigger": trigger.as_str(),
                            }))?
                        );
                    } else {
                        let report = engine.consolidate(trigger)?;
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    }
                }
                Command::RunDreamJobs { limit } => {
                    let reports = engine.run_pending_consolidation_jobs(limit)?;
                    println!("{}", serde_json::to_string_pretty(&reports)?);
                }
                Command::RefreshIndex { scope } => {
                    let scope = parse_scope(&scope)?;
                    let report = engine.refresh_pending_indexes(scope)?;
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

fn render_extraction_result(result: &ExtractionResult) -> Result<String> {
    Ok(serde_json::to_string_pretty(result)?)
}

fn render_ingest_preview(preview: &IngestPreview) -> Result<String> {
    Ok(serde_json::to_string_pretty(preview)?)
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

fn parse_recorded_at(raw: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    raw.map(|value| {
        DateTime::parse_from_rfc3339(value)
            .map(|ts| ts.with_timezone(&Utc))
            .map_err(Into::into)
    })
    .transpose()
}

fn parse_scope(raw: &str) -> Result<RebuildScope> {
    Ok(match raw.to_ascii_lowercase().as_str() {
        "all" => RebuildScope::All,
        "text" => RebuildScope::Text,
        "vector" => RebuildScope::Vector,
        _ => anyhow::bail!("invalid rebuild scope: {}", raw),
    })
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;
    use memo_engine::{ExtractedEntity, ExtractedFact, ExtractionResult};

    #[test]
    fn cli_parses_extract_command() {
        let cli = Cli::parse_from([
            "memo",
            "--data-dir",
            ".memo-test",
            "extract",
            "Alice lives in Paris.",
        ]);

        match cli.command {
            Command::Extract { content } => assert_eq!(content, "Alice lives in Paris."),
            _ => panic!("expected extract command"),
        }
    }

    #[test]
    fn render_extraction_result_outputs_pretty_json() {
        let output = super::render_extraction_result(&ExtractionResult {
            entities: vec![ExtractedEntity {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: vec!["Ally".to_string()],
                confidence: 0.9,
            }],
            facts: vec![ExtractedFact {
                subject: "Alice".to_string(),
                predicate: "lives_in".to_string(),
                object: "Paris".to_string(),
                confidence: 0.8,
            }],
        })
        .expect("expected JSON rendering");

        assert!(output.contains("\"entities\""));
        assert!(output.contains("\"lives_in\""));
        assert!(output.contains('\n'));
    }

    #[test]
    fn cli_parses_ingest_dry_run_flag() {
        let cli = Cli::parse_from(["memo", "ingest", "Alice lives in Paris.", "--dry-run"]);

        match cli.command {
            Command::Ingest {
                content, dry_run, ..
            } => {
                assert_eq!(content, "Alice lives in Paris.");
                assert!(dry_run);
            }
            _ => panic!("expected ingest command"),
        }
    }

    #[test]
    fn cli_parses_dream_enqueue_flag() {
        let cli = Cli::parse_from(["memo", "dream", "--trigger", "idle", "--enqueue"]);

        match cli.command {
            Command::Dream { trigger, enqueue } => {
                assert_eq!(trigger, "idle");
                assert!(enqueue);
            }
            _ => panic!("expected dream command"),
        }
    }

    #[test]
    fn cli_parses_run_dream_jobs_command() {
        let cli = Cli::parse_from(["memo", "run-dream-jobs", "--limit", "3"]);

        match cli.command {
            Command::RunDreamJobs { limit } => assert_eq!(limit, 3),
            _ => panic!("expected run-dream-jobs command"),
        }
    }

    #[test]
    fn cli_parses_refresh_index_command() {
        let cli = Cli::parse_from(["memo", "refresh-index", "--scope", "vector"]);

        match cli.command {
            Command::RefreshIndex { scope } => assert_eq!(scope, "vector"),
            _ => panic!("expected refresh-index command"),
        }
    }
}

use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use memo_engine::{
    DreamReport, DreamTrigger, EntityInput, EpisodeInput, ExtractionSource, FactInput, IndexStatus,
    MemoryEngine, MemoryRecord, RecallReason, RecallRequest, RecallResultSet, RememberPreview,
    RestoreReport, SystemState,
};
use serde::Serialize;

mod app_config;
mod lmkit_adapter;
mod lmkit_extraction_adapter;
mod lmkit_rerank_adapter;
mod provider_runtime;

const ACTIVE_DATA_DIR_FILE: &str = ".memo-home";
const MEMO_DATA_DIR_ENV: &str = "MEMO_DATA_DIR";

#[derive(Parser)]
#[command(name = "memo")]
#[command(about = "Local single-process memory engine")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Awaken {
        path: Option<PathBuf>,
    },
    Remember {
        content: String,
        #[arg(long = "time")]
        time: Option<String>,
        #[arg(long = "entity")]
        entities: Vec<String>,
        #[arg(long = "fact")]
        facts: Vec<String>,
        #[arg(long)]
        dry_run: bool,
        #[arg(long)]
        json: bool,
    },
    Recall {
        query: String,
        #[arg(short = 'n', long, default_value_t = 10)]
        limit: usize,
        #[arg(long)]
        deep: bool,
        #[arg(long)]
        json: bool,
    },
    Reflect {
        id: String,
        #[arg(long)]
        json: bool,
    },
    Dream {
        #[arg(long)]
        full: bool,
        #[arg(long)]
        json: bool,
    },
    State {
        #[arg(long)]
        json: bool,
    },
    Restore {
        #[arg(long)]
        full: bool,
        #[arg(long)]
        json: bool,
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

    let Cli { command } = Cli::parse();

    match command {
        Command::Awaken { path } => {
            let cwd = std::env::current_dir()?;
            let data_dir = path.unwrap_or_else(default_data_dir);
            let report = app_config::initialize_data_dir(&data_dir)?;
            remember_active_data_dir_for_cwd(&cwd, &data_dir)?;
            println!("{}", render_awaken_result(&data_dir, &report));
        }
        Command::Remember {
            content,
            time,
            entities,
            facts,
            dry_run,
            json,
        } => {
            let engine = open_engine()?;
            let input = EpisodeInput {
                content,
                layer: memo_engine::MemoryLayer::L1,
                entities: parse_entities(&entities)?,
                facts: parse_facts(&facts)?,
                source_episode_id: None,
                session_id: None,
                recorded_at: parse_recorded_at(time.as_deref())?,
                confidence: 0.85,
            };

            if dry_run {
                let preview = engine.preview_remember(&input)?;
                println!("{}", render_remember_preview(&preview, json)?);
                return Ok(());
            }

            let id = engine.remember(input)?;
            println!(
                "{}",
                render_json_or_text(&serde_json::json!({ "id": id }), &id, json)?
            );
        }
        Command::Recall {
            query,
            limit,
            deep,
            json,
        } => {
            let engine = open_engine()?;
            let result = engine.recall(RecallRequest { query, limit, deep })?;
            println!("{}", render_recall_result(&result, json)?);
        }
        Command::Reflect { id, json } => {
            let engine = open_engine()?;
            let record = engine.reflect(&id)?;
            println!("{}", render_reflection(&record, json)?);
        }
        Command::Dream { full, json } => {
            let engine = open_engine()?;
            let report = if full {
                engine.dream_full(DreamTrigger::Manual)?
            } else {
                engine.dream(DreamTrigger::Manual)?
            };
            println!("{}", render_dream_report(&report, full, json)?);
        }
        Command::State { json } => {
            let engine = open_engine()?;
            let state = engine.state()?;
            println!("{}", render_state(&state, json)?);
        }
        Command::Restore { full, json } => {
            let engine = open_engine()?;
            let report = if full {
                engine.restore_full(memo_engine::RestoreScope::All)?
            } else {
                engine.restore(memo_engine::RestoreScope::All)?
            };
            println!("{}", render_restore_report(&report, full, json)?);
        }
    }

    Ok(())
}

fn open_engine() -> Result<MemoryEngine> {
    let cwd = std::env::current_dir()?;
    let data_dir = resolve_data_dir_for_cwd(&cwd)?;
    let config = app_config::build_engine_config(data_dir)?;
    MemoryEngine::open(config)
}

fn default_data_dir() -> PathBuf {
    PathBuf::from(".memo")
}

fn resolve_data_dir_for_cwd(cwd: &Path) -> Result<PathBuf> {
    if let Some(value) = std::env::var_os(MEMO_DATA_DIR_ENV) {
        return Ok(resolve_relative_to_cwd(cwd, Path::new(&value)));
    }

    let active_path = cwd.join(ACTIVE_DATA_DIR_FILE);
    if active_path.exists() {
        let raw = fs::read_to_string(&active_path)?;
        let configured = PathBuf::from(raw.trim());
        if !configured.as_os_str().is_empty() {
            return Ok(resolve_relative_to_cwd(cwd, &configured));
        }
    }

    Ok(cwd.join(default_data_dir()))
}

fn remember_active_data_dir_for_cwd(cwd: &Path, data_dir: &Path) -> Result<()> {
    fs::write(
        cwd.join(ACTIVE_DATA_DIR_FILE),
        data_dir.to_string_lossy().as_ref(),
    )?;
    Ok(())
}

fn resolve_relative_to_cwd(cwd: &Path, path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        cwd.join(path)
    }
}

fn render_awaken_result(data_dir: &Path, report: &app_config::InitReport) -> String {
    format!(
        "Awakened memory space at {}\nconfig.toml: {}\nproviders.toml: {}",
        data_dir.display(),
        created_label(report.config_created),
        created_label(report.providers_created),
    )
}

fn render_remember_preview(preview: &RememberPreview, json: bool) -> Result<String> {
    let human = format!(
        "Remember preview\ncontent: {}\nlayer: {}\nentities: {}\nfacts: {}",
        preview.content,
        preview.layer.as_str(),
        preview.entities.len(),
        preview.facts.len(),
    );
    render_json_or_text(preview, &human, json)
}

fn render_recall_result(result: &RecallResultSet, json: bool) -> Result<String> {
    if json {
        return render_json_or_text(result, "", true);
    }

    let mut lines = vec![format!(
        "Recalled {} item(s) from {} candidate(s){}",
        result.results.len(),
        result.total_candidates,
        if result.deep_search_used {
            " with deep recall"
        } else {
            ""
        }
    )];

    for (index, item) in result.results.iter().enumerate() {
        lines.push(format!(
            "{}. [{}:{}] score={:.3} layer={}",
            index + 1,
            item.memory.kind(),
            item.memory.id(),
            item.score,
            item.memory.layer().as_str(),
        ));
        lines.push(format!("   {}", memory_summary(&item.memory)));
        if !item.reasons.is_empty() {
            lines.push(format!(
                "   reasons: {}",
                item.reasons
                    .iter()
                    .map(recall_reason_label)
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
    }

    Ok(lines.join("\n"))
}

fn render_reflection(record: &MemoryRecord, json: bool) -> Result<String> {
    if json {
        return render_json_or_text(record, "", true);
    }

    let mut lines = match record {
        MemoryRecord::Episode(episode) => vec![
            format!("Episode {}", episode.id),
            format!("layer: {}", episode.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!("confidence: {:.2}", episode.confidence),
            format!("content: {}", episode.content),
        ],
        MemoryRecord::Entity(entity) => vec![
            format!("Entity {}", entity.id),
            format!("layer: {}", entity.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!("name: {}", entity.canonical_name),
            format!(
                "aliases: {}",
                if entity.aliases.is_empty() {
                    "-".to_string()
                } else {
                    entity.aliases.join(", ")
                }
            ),
        ],
        MemoryRecord::Fact(fact) => vec![
            format!("Fact {}", fact.id),
            format!("layer: {}", fact.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!(
                "statement: {} {} {}",
                fact.subject_text, fact.predicate, fact.object_text
            ),
        ],
        MemoryRecord::Edge(edge) => vec![
            format!("Edge {}", edge.id),
            format!("layer: {}", edge.layer.as_str()),
            format!("status: {}", memory_status_label(record)),
            format!(
                "relation: {} {} {}",
                edge.subject_entity_id, edge.predicate, edge.object_entity_id
            ),
        ],
    };

    if let Some(archived_at) = memory_archived_at(record) {
        lines.push(format!("archived_at: {}", archived_at.to_rfc3339()));
    }
    if let Some(invalidated_at) = memory_invalidated_at(record) {
        lines.push(format!("invalidated_at: {}", invalidated_at.to_rfc3339()));
    }
    if let Some(valid_from) = memory_valid_from(record) {
        lines.push(format!("valid_from: {}", valid_from.to_rfc3339()));
    }
    if let Some(valid_to) = memory_valid_to(record) {
        lines.push(format!("valid_to: {}", valid_to.to_rfc3339()));
    }

    Ok(lines.join("\n"))
}

fn memory_status_label(record: &MemoryRecord) -> &'static str {
    if memory_invalidated_at(record).is_some() {
        "invalidated"
    } else if memory_archived_at(record).is_some() {
        "archived"
    } else {
        "active"
    }
}

fn memory_archived_at(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Episode(episode) => episode.archived_at,
        MemoryRecord::Entity(entity) => entity.archived_at,
        MemoryRecord::Fact(fact) => fact.archived_at,
        MemoryRecord::Edge(edge) => edge.archived_at,
    }
}

fn memory_invalidated_at(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Episode(episode) => episode.invalidated_at,
        MemoryRecord::Entity(entity) => entity.invalidated_at,
        MemoryRecord::Fact(fact) => fact.invalidated_at,
        MemoryRecord::Edge(edge) => edge.invalidated_at,
    }
}

fn memory_valid_from(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Fact(fact) => fact.valid_from,
        MemoryRecord::Edge(edge) => edge.valid_from,
        _ => None,
    }
}

fn memory_valid_to(record: &MemoryRecord) -> Option<DateTime<Utc>> {
    match record {
        MemoryRecord::Fact(fact) => fact.valid_to,
        MemoryRecord::Edge(edge) => edge.valid_to,
        _ => None,
    }
}

fn render_dream_report(report: &DreamReport, full: bool, json: bool) -> Result<String> {
    if json {
        let payload = serde_json::json!({
            "mode": if full { "full" } else { "standard" },
            "dream": report,
        });
        return render_json_or_text(&payload, "", true);
    }

    Ok(format!(
        "Dream {}complete\npasses_run: {}\nstructured_episodes: {}\nstructured_entities: {}\nstructured_facts: {}\nextraction_failures: {}\npromoted_to_l2: {}\npromoted_to_l3: {}\ndowngraded: {}\narchived: {}\ninvalidated: {}",
        if full { "(full) " } else { "" },
        report.passes_run,
        report.structured_episodes,
        report.structured_entities,
        report.structured_facts,
        report.extraction_failures,
        report.promoted_to_l2,
        report.promoted_to_l3,
        report.downgraded_records,
        report.archived_records,
        report.invalidated_records,
    ))
}

fn render_state(state: &SystemState, json: bool) -> Result<String> {
    if json {
        return render_json_or_text(state, "", true);
    }

    Ok(format!(
        "State\nrecords: episodes={} entities={} facts={} edges={}\nlayers: l1={} l2={} l3={} archived={} invalidated={}\nl3_cached: {}\ntext_index: {}\nvector_index: {}",
        state.episode_count,
        state.entity_count,
        state.fact_count,
        state.edge_count,
        state.layers.l1,
        state.layers.l2,
        state.layers.l3,
        state.layers.archived,
        state.layers.invalidated,
        state.l3_cached,
        index_summary(&state.text_index),
        index_summary(&state.vector_index),
    ))
}

fn render_restore_report(report: &RestoreReport, full: bool, json: bool) -> Result<String> {
    if json {
        let payload = serde_json::json!({
            "mode": if full { "full" } else { "standard" },
            "restore": report,
        });
        return render_json_or_text(&payload, "", true);
    }

    Ok(format!(
        "Restore {}complete\ntext_documents: {}\nvector_documents: {}",
        if full { "(full) " } else { "" },
        report.text_documents,
        report.vector_documents,
    ))
}

fn render_json_or_text<T: Serialize>(value: &T, human: &str, json: bool) -> Result<String> {
    if json {
        Ok(serde_json::to_string_pretty(value)?)
    } else {
        Ok(human.to_string())
    }
}

fn created_label(created: bool) -> &'static str {
    if created {
        "created"
    } else {
        "kept"
    }
}

fn memory_summary(memory: &MemoryRecord) -> String {
    match memory {
        MemoryRecord::Episode(episode) => episode.content.clone(),
        MemoryRecord::Entity(entity) => entity.canonical_name.clone(),
        MemoryRecord::Fact(fact) => {
            format!(
                "{} {} {}",
                fact.subject_text, fact.predicate, fact.object_text
            )
        }
        MemoryRecord::Edge(edge) => {
            format!(
                "{} {} {}",
                edge.subject_entity_id, edge.predicate, edge.object_entity_id
            )
        }
    }
}

fn recall_reason_label(reason: &RecallReason) -> String {
    match reason {
        RecallReason::L0 => "l0".to_string(),
        RecallReason::L3 => "l3".to_string(),
        RecallReason::Exact => "exact".to_string(),
        RecallReason::Alias => "alias".to_string(),
        RecallReason::Bm25 => "bm25".to_string(),
        RecallReason::Vector => "vector".to_string(),
        RecallReason::Rerank => "rerank".to_string(),
        RecallReason::GraphHop { hops } => format!("graph_hop({hops})"),
        RecallReason::RecencyBoost => "recency_boost".to_string(),
        RecallReason::LayerBoost => "layer_boost".to_string(),
        RecallReason::HitFrequencyBoost => "hit_frequency_boost".to_string(),
        RecallReason::MmrSelected => "mmr_selected".to_string(),
    }
}

fn index_summary(index: &IndexStatus) -> String {
    let mut segments = vec![format!("{} docs={}", index.status, index.doc_count)];
    if index.pending_updates > 0 {
        segments.push(format!("pending_updates={}", index.pending_updates));
    }
    if index.failed_updates > 0 {
        segments.push(format!("failed_updates={}", index.failed_updates));
    }
    if index.failed_attempts_max > 0 {
        segments.push(format!("failed_attempts_max={}", index.failed_attempts_max));
    }
    if let Some(last_error) = index.last_error.as_deref() {
        segments.push(format!("last_error={last_error}"));
    }
    if let Some(detail) = index.detail.as_deref() {
        segments.push(format!("detail={detail}"));
    }
    segments.join(" ")
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

fn parse_recorded_at(raw: Option<&str>) -> Result<Option<DateTime<Utc>>> {
    raw.map(|value| {
        DateTime::parse_from_rfc3339(value)
            .map(|ts| ts.with_timezone(&Utc))
            .map_err(Into::into)
    })
    .transpose()
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        path::Path,
    };

    use chrono::{TimeZone, Utc};
    use tempfile::TempDir;

    use super::{
        remember_active_data_dir_for_cwd, render_dream_report, render_recall_result,
        render_reflection, render_state, resolve_data_dir_for_cwd, Cli, Command,
    };
    use clap::Parser;
    use memo_engine::{
        DreamReport, EpisodeRecord, FactRecord, IndexStatus, MemoryLayer, MemoryRecord,
        RecallReason, RecallResult, RecallResultSet, SystemState,
    };

    #[test]
    fn cli_parses_awaken_with_optional_path() {
        let cli = Cli::parse_from(["memo", "awaken", ".memo-test"]);

        match cli.command {
            Command::Awaken { path } => {
                assert_eq!(
                    path.expect("expected path"),
                    std::path::PathBuf::from(".memo-test")
                );
            }
            _ => panic!("expected awaken command"),
        }
    }

    #[test]
    fn cli_parses_remember_dry_run_flag() {
        let cli = Cli::parse_from(["memo", "remember", "Alice lives in Paris.", "--dry-run"]);

        match cli.command {
            Command::Remember {
                content, dry_run, ..
            } => {
                assert_eq!(content, "Alice lives in Paris.");
                assert!(dry_run);
            }
            _ => panic!("expected remember command"),
        }
    }

    #[test]
    fn cli_parses_recall_deep_flag() {
        let cli = Cli::parse_from(["memo", "recall", "Alice", "--deep"]);

        match cli.command {
            Command::Recall { query, deep, .. } => {
                assert_eq!(query, "Alice");
                assert!(deep);
            }
            _ => panic!("expected recall command"),
        }
    }

    #[test]
    fn cli_parses_dream_full_flag() {
        let cli = Cli::parse_from(["memo", "dream", "--full"]);

        match cli.command {
            Command::Dream { full, .. } => assert!(full),
            _ => panic!("expected dream command"),
        }
    }

    #[test]
    fn cli_parses_restore_full_flag() {
        let cli = Cli::parse_from(["memo", "restore", "--full"]);

        match cli.command {
            Command::Restore { full, .. } => assert!(full),
            _ => panic!("expected restore command"),
        }
    }

    #[test]
    fn resolve_data_dir_defaults_to_workspace_memo_dir() -> anyhow::Result<()> {
        let temp = TempDir::new()?;

        let resolved = resolve_data_dir_for_cwd(temp.path())?;

        assert_eq!(resolved, temp.path().join(".memo"));
        Ok(())
    }

    #[test]
    fn resolve_data_dir_uses_workspace_active_file() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        remember_active_data_dir_for_cwd(temp.path(), Path::new("custom-store"))?;

        let resolved = resolve_data_dir_for_cwd(temp.path())?;

        assert_eq!(resolved, temp.path().join("custom-store"));
        Ok(())
    }

    #[test]
    fn resolve_data_dir_prefers_environment_override() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        let override_path = temp.path().join("env-store");
        remember_active_data_dir_for_cwd(temp.path(), Path::new("custom-store"))?;
        unsafe {
            std::env::set_var("MEMO_DATA_DIR", override_path.as_os_str());
        }

        let resolved = resolve_data_dir_for_cwd(temp.path())?;

        unsafe {
            std::env::remove_var("MEMO_DATA_DIR");
        }
        assert_eq!(resolved, override_path);
        Ok(())
    }

    #[test]
    fn remember_active_data_dir_overwrites_previous_workspace_target() -> anyhow::Result<()> {
        let temp = TempDir::new()?;
        remember_active_data_dir_for_cwd(temp.path(), Path::new(".memo"))?;
        remember_active_data_dir_for_cwd(temp.path(), Path::new("custom-store"))?;

        let active = fs::read_to_string(temp.path().join(".memo-home"))?;

        assert_eq!(active.trim(), "custom-store");
        Ok(())
    }

    #[test]
    fn render_state_without_json_uses_human_summary() {
        let output = render_state(
            &SystemState {
                episode_count: 3,
                entity_count: 2,
                fact_count: 1,
                edge_count: 1,
                l3_cached: 4,
                layers: memo_engine::LayerSummary {
                    l1: 2,
                    l2: 1,
                    l3: 0,
                    archived: 3,
                    invalidated: 1,
                },
                text_index: IndexStatus {
                    name: "text".to_string(),
                    doc_count: 8,
                    status: "ready".to_string(),
                    detail: None,
                    pending_updates: 0,
                    failed_updates: 0,
                    failed_attempts_max: 0,
                    last_error: None,
                },
                vector_index: IndexStatus {
                    name: "vector".to_string(),
                    doc_count: 5,
                    status: "failed".to_string(),
                    detail: Some("restore failed for queued updates".to_string()),
                    pending_updates: 0,
                    failed_updates: 2,
                    failed_attempts_max: 3,
                    last_error: Some("vector dimension mismatch".to_string()),
                },
            },
            false,
        )
        .expect("expected human state output");

        assert!(output.contains("State"));
        assert!(output.contains("layers: l1=2 l2=1 l3=0 archived=3 invalidated=1"));
        assert!(output.contains("vector_index: failed docs=5"));
        assert!(output.contains("failed_updates=2"));
        assert!(output.contains("failed_attempts_max=3"));
        assert!(output.contains("last_error=vector dimension mismatch"));
        assert!(!output.contains("dream_jobs"));
    }

    #[test]
    fn render_recall_without_json_summarizes_results() {
        let output = render_recall_result(
            &RecallResultSet {
                total_candidates: 2,
                deep_search_used: true,
                results: vec![RecallResult {
                    memory: MemoryRecord::Episode(EpisodeRecord {
                        id: "ep-1".to_string(),
                        content: "Alice lives in Paris.".to_string(),
                        layer: MemoryLayer::L2,
                        confidence: 0.9,
                        source_episode_id: None,
                        session_id: None,
                        created_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                        updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                        last_seen_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                        archived_at: None,
                        invalidated_at: None,
                        hit_count: 3,
                    }),
                    score: 3.4,
                    reasons: vec![RecallReason::Alias, RecallReason::LayerBoost],
                }],
            },
            false,
        )
        .expect("expected human recall output");

        assert!(output.contains("Recalled 1 item(s)"));
        assert!(output.contains("[episode:ep-1] score=3.400 layer=L2"));
        assert!(output.contains("reasons: alias, layer_boost"));
    }

    #[test]
    fn render_reflection_marks_archived_episode_status() {
        let output = render_reflection(
            &MemoryRecord::Episode(EpisodeRecord {
                id: "ep-archived".to_string(),
                content: "Alice archived note.".to_string(),
                layer: MemoryLayer::L2,
                confidence: 0.9,
                source_episode_id: None,
                session_id: None,
                created_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 11, 0, 0).unwrap(),
                last_seen_at: Utc.with_ymd_and_hms(2026, 4, 21, 11, 0, 0).unwrap(),
                archived_at: Some(Utc.with_ymd_and_hms(2026, 4, 21, 12, 0, 0).unwrap()),
                invalidated_at: None,
                hit_count: 3,
            }),
            false,
        )
        .expect("expected reflection output");

        assert!(output.contains("status: archived"));
        assert!(output.contains("archived_at: 2026-04-21T12:00:00+00:00"));
    }

    #[test]
    fn render_reflection_marks_invalidated_fact_window() {
        let output = render_reflection(
            &MemoryRecord::Fact(FactRecord {
                id: "fact-1".to_string(),
                subject_entity_id: Some("alice".to_string()),
                subject_text: "Alice".to_string(),
                predicate: "lives_in".to_string(),
                object_entity_id: Some("paris".to_string()),
                object_text: "Paris".to_string(),
                layer: MemoryLayer::L2,
                confidence: 0.8,
                source_episode_id: Some("ep-1".to_string()),
                created_at: Utc.with_ymd_and_hms(2026, 4, 21, 10, 0, 0).unwrap(),
                updated_at: Utc.with_ymd_and_hms(2026, 4, 21, 13, 0, 0).unwrap(),
                valid_from: Some(Utc.with_ymd_and_hms(2026, 4, 20, 9, 0, 0).unwrap()),
                valid_to: Some(Utc.with_ymd_and_hms(2026, 4, 21, 13, 0, 0).unwrap()),
                archived_at: None,
                invalidated_at: Some(Utc.with_ymd_and_hms(2026, 4, 21, 13, 0, 0).unwrap()),
                hit_count: 2,
            }),
            false,
        )
        .expect("expected reflection output");

        assert!(output.contains("status: invalidated"));
        assert!(output.contains("invalidated_at: 2026-04-21T13:00:00+00:00"));
        assert!(output.contains("valid_from: 2026-04-20T09:00:00+00:00"));
        assert!(output.contains("valid_to: 2026-04-21T13:00:00+00:00"));
    }

    #[test]
    fn render_state_with_json_returns_json() {
        let output = render_state(
            &SystemState {
                text_index: IndexStatus {
                    name: "text".to_string(),
                    ..Default::default()
                },
                vector_index: IndexStatus {
                    name: "vector".to_string(),
                    ..Default::default()
                },
                ..Default::default()
            },
            true,
        )
        .expect("expected json state output");

        let parsed: serde_json::Value =
            serde_json::from_str(&output).expect("expected valid json output");
        assert!(parsed.get("dream_jobs").is_none());
        assert_eq!(parsed["layers"]["l1"], 0);
        assert_eq!(parsed["layers"]["archived"], 0);
    }

    #[test]
    fn render_full_dream_report_uses_pass_count() {
        let output = render_dream_report(
            &DreamReport {
                passes_run: 2,
                promoted_to_l2: 3,
                promoted_to_l3: 1,
                downgraded_records: 0,
                archived_records: 2,
                invalidated_records: 1,
                ..Default::default()
            },
            true,
            false,
        )
        .expect("expected human dream output");

        assert!(output.contains("Dream (full) complete"));
        assert!(output.contains("passes_run: 2"));
    }
}

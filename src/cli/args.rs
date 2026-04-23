use anyhow::Result;
use chrono::{DateTime, Utc};
use clap::{Parser, Subcommand};
use memo_engine::{EntityInput, EpisodeInput, ExtractionSource, FactInput, MemoryLayer};

#[derive(Debug, Parser)]
#[command(name = "memo")]
#[command(about = "Local single-process memory engine")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    Awaken,
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

pub(crate) fn build_remember_input(
    content: String,
    time: Option<String>,
    entities: &[String],
    facts: &[String],
) -> Result<EpisodeInput> {
    Ok(EpisodeInput {
        content,
        layer: MemoryLayer::L1,
        entities: parse_entities(entities)?,
        facts: parse_facts(facts)?,
        source_episode_id: None,
        session_id: None,
        recorded_at: parse_recorded_at(time.as_deref())?,
        confidence: 0.85,
    })
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
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn cli_parses_awaken_without_path_argument() {
        let cli = Cli::parse_from(["memo", "awaken"]);

        match cli.command {
            Command::Awaken => {}
            _ => panic!("expected awaken command"),
        }
    }

    #[test]
    fn cli_rejects_custom_awaken_path_argument() {
        let error = Cli::try_parse_from(["memo", "awaken", ".memo-test"])
            .expect_err("expected awaken path argument to be rejected");

        assert!(error.to_string().contains("unexpected argument"));
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
    fn cli_parses_reflect_json_flag() {
        let cli = Cli::parse_from(["memo", "reflect", "ep-1", "--json"]);

        match cli.command {
            Command::Reflect { id, json } => {
                assert_eq!(id, "ep-1");
                assert!(json);
            }
            _ => panic!("expected reflect command"),
        }
    }

    #[test]
    fn cli_parses_state_json_flag() {
        let cli = Cli::parse_from(["memo", "state", "--json"]);

        match cli.command {
            Command::State { json } => assert!(json),
            _ => panic!("expected state command"),
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
}

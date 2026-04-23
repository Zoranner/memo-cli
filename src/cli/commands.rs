use std::path::PathBuf;

use anyhow::Result;
use memo_engine::{DreamTrigger, MemoryEngine, RecallRequest, RestoreScope};

use crate::{
    cli::{
        args::{build_remember_input, Cli, Command},
        output::{
            render_awaken_result, render_dream_report, render_json_or_text, render_recall_result,
            render_reflection, render_remember_preview, render_restore_report, render_state,
        },
        paths::{default_config_dir, resolve_data_dir_for_config_dir},
    },
    config,
    providers::status,
};

pub(crate) fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Command::Awaken => {
            let config_dir = default_config_dir()?;
            let data_dir = resolve_data_dir_for_config_dir(&config_dir)?;
            let report = config::initialize_app_home(&config_dir, &data_dir)?;
            println!("{}", render_awaken_result(&data_dir, &config_dir, &report));
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
            let input = build_remember_input(content, time, &entities, &facts)?;

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
            let (engine, data_dir) = open_engine_with_data_dir()?;
            let state = engine.state()?;
            let provider_runtime = status::load_provider_runtime_summary(&data_dir);
            println!("{}", render_state(&state, &provider_runtime, json)?);
        }
        Command::Restore { full, json } => {
            let engine = open_engine()?;
            let report = if full {
                engine.restore_full(RestoreScope::All)?
            } else {
                engine.restore(RestoreScope::All)?
            };
            println!("{}", render_restore_report(&report, full, json)?);
        }
    }

    Ok(())
}

fn open_engine() -> Result<MemoryEngine> {
    Ok(open_engine_with_data_dir()?.0)
}

fn open_engine_with_data_dir() -> Result<(MemoryEngine, PathBuf)> {
    let config_dir = default_config_dir()?;
    let data_dir = resolve_data_dir_for_config_dir(&config_dir)?;
    let config = config::build_engine_config(&data_dir, &config_dir)?;
    Ok((MemoryEngine::open(config)?, data_dir))
}

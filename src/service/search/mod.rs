mod engine;
mod multi;
mod subquery_merge;
mod types;

use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use std::sync::Arc;

use crate::config::ProvidersConfig;
use crate::service::context::{open_local_embed_session, LocalEmbedSession};
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, TimeRange};

pub struct SearchOptions {
    pub query: String,
    pub limit: usize,
    pub threshold: f32,
    pub after: Option<String>,
    pub before: Option<String>,
    pub force_local: bool,
    pub force_global: bool,
}

pub async fn search(options: SearchOptions) -> Result<()> {
    let SearchOptions {
        query,
        limit,
        threshold,
        after,
        before,
        force_local,
        force_global,
    } = options;
    let output = Output::new();

    let (session, _) = open_local_embed_session(force_local, force_global).await?;
    let LocalEmbedSession {
        config,
        storage,
        embed_provider,
        brain_path,
    } = session;

    let storage: Arc<LocalStorageClient> = Arc::new(storage);
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    let time_range = build_time_range(after, before)?;
    let providers = ProvidersConfig::load()?;
    let rerank_config = config.resolve_rerank(&providers)?;
    let llm_config = config.resolve_llm(&providers)?;

    let (results, summary) = multi::search(
        multi::MultiSearchOptions {
            query: query.clone(),
            limit,
            threshold,
            time_range,
            storage,
            embed_provider,
            rerank_config,
            llm_config,
            app_config: config,
        },
        &output,
    )
    .await?;

    if results.is_empty() {
        output.info(&format!(
            "No results found above threshold {:.2}",
            threshold
        ));
        output.note("Try lowering the threshold with -t/--threshold option");
    } else {
        if let Some(text) = &summary {
            output.llm_answer(text);
        }
        output.search_results(&results);
    }

    Ok(())
}

fn build_time_range(after: Option<String>, before: Option<String>) -> Result<Option<TimeRange>> {
    if after.is_none() && before.is_none() {
        return Ok(None);
    }

    let after_ts = after.as_ref().map(|s| parse_datetime(s)).transpose()?;
    let before_ts = before.as_ref().map(|s| parse_datetime(s)).transpose()?;

    Ok(Some(TimeRange {
        after: after_ts,
        before: before_ts,
    }))
}

fn parse_datetime(input: &str) -> Result<i64> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M") {
        return Ok(dt.and_utc().timestamp_millis());
    }

    if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let dt = date
            .and_hms_opt(0, 0, 0)
            .context("Failed to create datetime")?;
        return Ok(dt.and_utc().timestamp_millis());
    }

    anyhow::bail!("Invalid date format. Use YYYY-MM-DD or YYYY-MM-DD HH:MM")
}

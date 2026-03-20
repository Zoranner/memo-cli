mod engine;
mod multi;
mod subquery_merge;
mod types;

use anyhow::Result;
use std::sync::Arc;

use crate::service::session::{open_local_embed_session, LocalEmbedSession};
use crate::service::time_range::parse_cli_time_range;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::StorageBackend;

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
        providers,
        storage,
        embed_provider,
        brain_path,
        ..
    } = session;

    let storage: Arc<LocalStorageClient> = Arc::new(storage);
    let record_count = storage.count().await?;

    output.database_info(&brain_path, record_count);

    let time_range = parse_cli_time_range(after, before)?;
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

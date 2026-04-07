mod engine;
mod multi;
mod subquery_merge;
mod types;

use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

use crate::llm::{summarize_results_stream, LlmClient};
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
    let decompose_llm_config = config.resolve_decompose_llm(&providers)?;
    let summarize_llm_config = config.resolve_summarize_llm(&providers)?;
    let summarize_strategy_owned = config.summarize.strategy_prompt.clone();

    let t_total = Instant::now();

    let results = multi::search(
        multi::MultiSearchOptions {
            query: query.clone(),
            limit,
            threshold,
            time_range,
            storage,
            embed_provider,
            rerank_config,
            decompose_llm_config,
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
        output.finished(t_total.elapsed());
        return Ok(());
    }

    // 先流式输出 LLM 总结（用户立即开始阅读）
    let summarize_client = LlmClient::from_resolved(&summarize_llm_config)?;
    output.status("Summarizing", "results with LLM");
    match summarize_results_stream(
        &summarize_client,
        &query,
        &results,
        summarize_strategy_owned.as_deref(),
    )
    .await
    {
        Ok(stream) => output.llm_answer_stream(stream).await,
        Err(e) => tracing::debug!("LLM summarization failed: {}", e),
    }

    // 总结结束后，以引用格式展示原始记录
    output.search_results_brief(&results);

    output.finished(t_total.elapsed());

    Ok(())
}

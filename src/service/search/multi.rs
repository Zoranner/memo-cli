use anyhow::Result;
use futures::future::join_all;
use std::sync::Arc;

use crate::config::{AppConfig, ResolvedService};
use crate::llm::{summarize_results, LlmClient};
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{QueryResult, TimeRange};
use model_provider::EmbedProvider;

use super::decompose::build_decomposition_tree;
use super::engine::{multi_layer_search, LayerSearchParams};
use super::merge::merge_results;
use super::types::SubQueryResult;

pub struct MultiSearchOptions {
    pub query: String,
    pub limit: usize,
    pub threshold: f32,
    pub time_range: Option<TimeRange>,
    pub storage: LocalStorageClient,
    pub embed_provider: Box<dyn EmbedProvider>,
    pub rerank_config: ResolvedService,
    pub llm_config: ResolvedService,
    pub app_config: AppConfig,
}

/// 执行多查询搜索，返回 (结果列表, LLM 总结)
pub async fn search(
    options: MultiSearchOptions,
    output: &Output,
) -> Result<(Vec<QueryResult>, Option<String>)> {
    let MultiSearchOptions {
        query,
        limit,
        threshold,
        time_range,
        storage,
        embed_provider,
        rerank_config,
        llm_config,
        app_config,
    } = options;

    let decomp_config = &app_config.decomposition;
    let mq_config = &app_config.multi_query;
    let prompts = &app_config.prompts;

    let llm_client = LlmClient::from_resolved(&llm_config)?;

    output.status(
        "Decomposing",
        &format!(
            "query into sub-questions (max_level={})",
            decomp_config.max_level
        ),
    );
    let tree = build_decomposition_tree(
        &query,
        &llm_client,
        decomp_config,
        prompts.decompose.as_deref(),
    )
    .await?;
    let leaves = tree.get_leaves();

    if leaves.is_empty() {
        output.info("Decomposition produced no sub-questions");
        return Ok((Vec::new(), None));
    }

    output.status("Decomposed", &format!("{} sub-questions", leaves.len()));
    output.status(
        "Searching",
        &format!("{} sub-queries in parallel", leaves.len()),
    );

    let embed_provider: Arc<dyn EmbedProvider> = Arc::from(embed_provider);
    let storage = Arc::new(storage);

    let search_tasks: Vec<_> = leaves
        .iter()
        .map(|leaf| {
            let leaf_query = leaf.query.clone();
            let leaf_id = leaf.id.clone();
            let time_range = time_range.clone();
            let rerank_config = rerank_config.clone();
            let candidates_limit = mq_config.candidates_per_query;
            let top_n = mq_config.top_n_per_leaf;
            let embed_provider = Arc::clone(&embed_provider);
            let storage = Arc::clone(&storage);

            async move {
                let query_vector = match embed_provider.encode(&leaf_query).await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::warn!("Failed to encode leaf query '{}': {}", leaf_query, e);
                        return None;
                    }
                };

                let params = LayerSearchParams {
                    query_vector,
                    query: &leaf_query,
                    limit: candidates_limit,
                    threshold,
                    time_range,
                    storage: &storage,
                    rerank_config: &rerank_config,
                    output: &Output::new(),
                };

                match multi_layer_search(params).await {
                    Ok(results) => Some(SubQueryResult {
                        node_id: leaf_id,
                        results: results.into_iter().take(top_n).collect(),
                    }),
                    Err(e) => {
                        tracing::warn!("Leaf search failed: {}", e);
                        None
                    }
                }
            }
        })
        .collect();

    let sub_results: Vec<SubQueryResult> =
        join_all(search_tasks).await.into_iter().flatten().collect();

    if sub_results.is_empty() {
        output.info("No results found in sub-queries");
        return Ok((Vec::new(), None));
    }

    output.status(
        "Merging",
        &format!("results from {} sub-queries", sub_results.len()),
    );
    let merged = merge_results(sub_results, mq_config);

    let result_limit = limit.min(mq_config.max_total_results);
    let final_memories: Vec<QueryResult> = merged
        .iter()
        .take(result_limit)
        .map(|m| m.memory.clone())
        .collect();

    output.status(
        "Results",
        &format!("{} results from multi-query search", final_memories.len()),
    );

    // LLM 综合总结
    let summary = if final_memories.is_empty() {
        None
    } else {
        output.status("Summarizing", "results with LLM");
        match summarize_results(
            &llm_client,
            &query,
            &final_memories,
            prompts.summarize.as_deref(),
        )
        .await
        {
            Ok(text) => Some(text),
            Err(e) => {
                tracing::warn!("LLM summarization failed: {}", e);
                None
            }
        }
    };

    Ok((final_memories, summary))
}

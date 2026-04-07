use anyhow::Result;
use futures::future::join_all;
use std::collections::HashSet;
use std::sync::Arc;

use crate::config::{AppConfig, ResolvedService};
use crate::llm::{decompose_query_tree, LlmClient};
use crate::ui::Output;
use lmkit::{create_rerank_provider, EmbedProvider, RerankProvider};
use memo_local::LocalStorageClient;
use memo_types::{QueryResult, StorageBackend, TimeRange};

use super::engine::apply_rerank;
use super::subquery_merge::merge_results;
use super::types::SubQueryResult;

pub struct MultiSearchOptions {
    pub query: String,
    pub limit: usize,
    pub threshold: f32,
    pub time_range: Option<TimeRange>,
    pub storage: Arc<LocalStorageClient>,
    pub embed_provider: Box<dyn EmbedProvider>,
    pub rerank_config: ResolvedService,
    pub decompose_llm_config: ResolvedService,
    pub app_config: AppConfig,
}

/// 执行多查询搜索，返回最终结果列表
pub async fn search(options: MultiSearchOptions, output: &Output) -> Result<Vec<QueryResult>> {
    let MultiSearchOptions {
        query,
        limit,
        threshold,
        time_range,
        storage,
        embed_provider,
        rerank_config,
        decompose_llm_config,
        app_config,
    } = options;

    let decomp_config = &app_config.decompose;
    let merge_config = &app_config.merge;

    let decompose_strategy = decomp_config.strategy_prompt.as_deref();
    let decompose_llm_client = LlmClient::from_resolved(&decompose_llm_config)?;

    output.status("Decomposing", "query into sub-questions");

    // 并发：decompose + 原始 query embed，节省一次串行网络等待
    let (trees_result, original_vec_result) = tokio::join!(
        decompose_query_tree(&decompose_llm_client, &query, decompose_strategy),
        embed_provider.encode(&query)
    );
    let trees = trees_result?;
    let original_vec = original_vec_result.ok();

    let mut seen = HashSet::new();
    let leaves: Vec<String> = trees
        .iter()
        .flat_map(|t| t.leaves())
        .filter(|q| seen.insert(q.clone()))
        .take(decomp_config.max_leaves)
        .collect();

    if leaves.is_empty() {
        output.info("Decomposition produced no sub-questions");
        return Ok(Vec::new());
    }

    output.status("Decomposed", &format!("{} sub-questions", leaves.len()));

    let embed_provider: Arc<dyn EmbedProvider> = Arc::from(embed_provider);
    let rerank_pc = rerank_config.to_provider_config(None);
    let rerank_shared: Arc<dyn RerankProvider> = Arc::from(create_rerank_provider(&rerank_pc)?);

    let candidates_limit = merge_config.candidates_per_query;
    let top_n = merge_config.results_per_leaf;

    // 如果原始 query 不在叶节点中，将其作为额外一路加入（使用已有向量，无需额外 embed）
    let original_query_as_leaf = if !leaves.contains(&query) {
        original_vec.map(|v| (query.clone(), v))
    } else {
        None
    };

    output.status(
        "Searching",
        &format!(
            "{} sub-queries in parallel",
            leaves.len() + original_query_as_leaf.as_ref().map_or(0, |_| 1)
        ),
    );

    // 叶节点搜索任务（需要 embed）
    let leaf_tasks: Vec<_> = leaves
        .iter()
        .enumerate()
        .map(|(idx, leaf_query)| {
            let leaf_query = leaf_query.clone();
            let leaf_id = format!("leaf_{}", idx);
            let time_range = time_range.clone();
            let embed_provider = Arc::clone(&embed_provider);
            let storage = Arc::clone(&storage);

            async move {
                let query_vector = match embed_provider.encode(&leaf_query).await {
                    Ok(v) => v,
                    Err(e) => {
                        tracing::debug!("Failed to encode leaf query '{}': {}", leaf_query, e);
                        return None;
                    }
                };

                match storage
                    .search_by_vector(query_vector, candidates_limit, threshold, time_range)
                    .await
                {
                    Ok(results) => Some(SubQueryResult {
                        node_id: leaf_id,
                        results: results.into_iter().take(top_n).collect::<Vec<_>>(),
                    }),
                    Err(e) => {
                        tracing::debug!("Leaf search failed: {}", e);
                        None
                    }
                }
            }
        })
        .collect();

    let mut sub_results: Vec<SubQueryResult> =
        join_all(leaf_tasks).await.into_iter().flatten().collect();

    // 原始 query 额外一路（向量已有，直接搜索）
    if let Some((_orig_query, orig_vec)) = original_query_as_leaf {
        match storage
            .search_by_vector(orig_vec, candidates_limit, threshold, time_range.clone())
            .await
        {
            Ok(results) => sub_results.push(SubQueryResult {
                node_id: "original".to_string(),
                results: results.into_iter().take(top_n).collect::<Vec<_>>(),
            }),
            Err(e) => tracing::debug!("Original query search failed: {}", e),
        }
    }

    if sub_results.is_empty() {
        output.info("No results found in sub-queries");
        return Ok(Vec::new());
    }

    output.status(
        "Merging",
        &format!("results from {} sub-queries", sub_results.len()),
    );
    let merged = merge_results(sub_results, merge_config);

    let result_limit = limit.min(merge_config.max_results);
    let candidates: Vec<QueryResult> = merged
        .into_iter()
        .take(merge_config.max_results)
        .map(|m| m.memory)
        .collect();

    // 全局一次 rerank（替代原来每个叶节点各自 rerank）
    let final_memories =
        apply_rerank(candidates, &query, result_limit, rerank_shared, output).await?;

    output.status(
        "Results",
        &format!("{} results from multi-query search", final_memories.len()),
    );

    Ok(final_memories)
}

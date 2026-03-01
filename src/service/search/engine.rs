use anyhow::Result;
use futures::future::join_all;
use std::collections::HashSet;

use crate::config::ResolvedService;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{QueryResult, ScoreType, SearchConfig as MultiLayerSearchConfig, StorageBackend};

/// 多层搜索参数
pub struct LayerSearchParams<'a> {
    pub query_vector: Vec<f32>,
    pub query: &'a str,
    pub limit: usize,
    pub threshold: f32,
    pub time_range: Option<memo_types::TimeRange>,
    pub storage: &'a LocalStorageClient,
    pub rerank_config: &'a ResolvedService,
    pub output: &'a Output,
}

/// 执行多层向量搜索 + 智能重排序，返回最终结果列表
pub async fn multi_layer_search(params: LayerSearchParams<'_>) -> Result<Vec<QueryResult>> {
    let LayerSearchParams {
        query_vector,
        query,
        limit,
        threshold,
        time_range,
        storage,
        rerank_config,
        output,
    } = params;

    let max_nodes = if limit < 10 { 50 } else { limit * 10 };
    let search_config = MultiLayerSearchConfig::new(threshold, max_nodes);
    let thresholds = search_config.generate_thresholds();
    let max_layers = thresholds.len().min(search_config.max_depth);

    let mut visited = HashSet::new();
    let mut all_candidates = Vec::new();

    output.status("Searching", "layer 1");
    let mut current_layer_results = storage
        .search_by_vector(
            query_vector,
            search_config.branch_limit,
            thresholds[0],
            time_range.clone(),
        )
        .await?;

    if current_layer_results.is_empty() {
        return Ok(Vec::new());
    }

    for result in &current_layer_results {
        if visited.insert(result.id.clone()) {
            all_candidates.push(result.clone());
        }
    }

    for (layer_index, &layer_threshold) in
        thresholds.iter().enumerate().skip(1).take(max_layers - 1)
    {
        if all_candidates.len() >= max_nodes || current_layer_results.is_empty() {
            break;
        }

        output.status("Searching", &format!("layer {}", layer_index + 1));

        let search_tasks: Vec<_> = current_layer_results
            .iter()
            .map(|result| {
                let result_id = result.id.clone();
                let time_range = time_range.clone();
                let branch_limit = search_config.branch_limit;
                let require_tag_overlap = search_config.require_tag_overlap;

                async move {
                    let memory = storage.find_memory_by_id(&result_id).await?;
                    let memory = match memory {
                        Some(m) => m,
                        None => return Ok::<Vec<QueryResult>, anyhow::Error>(Vec::new()),
                    };

                    let mut related = storage
                        .search_by_vector(
                            memory.vector.clone(),
                            branch_limit * 2,
                            layer_threshold,
                            time_range,
                        )
                        .await?;

                    if require_tag_overlap {
                        related.retain(|r| r.tags.iter().any(|t| memory.tags.contains(t)));
                    }

                    related.truncate(branch_limit);
                    Ok(related)
                }
            })
            .collect();

        let all_related = join_all(search_tasks).await;

        let mut next_layer_results = Vec::new();
        for related_result in all_related {
            match related_result {
                Ok(related) => {
                    for rel in related {
                        if visited.insert(rel.id.clone()) {
                            all_candidates.push(rel.clone());
                            next_layer_results.push(rel);
                            if all_candidates.len() >= max_nodes {
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::debug!("Branch search failed: {}", e);
                    continue;
                }
            }
            if all_candidates.len() >= max_nodes {
                break;
            }
        }

        current_layer_results = next_layer_results;
    }

    apply_rerank(all_candidates, query, limit, rerank_config, output).await
}

async fn apply_rerank(
    all_candidates: Vec<QueryResult>,
    query: &str,
    limit: usize,
    rerank_config: &ResolvedService,
    output: &Output,
) -> Result<Vec<QueryResult>> {
    if !should_use_rerank(&all_candidates, limit) {
        output.status("Ranking", "by vector similarity (rerank skipped)");
        let mut sorted = all_candidates;
        sorted.sort_by(|a, b| {
            b.score
                .unwrap_or(0.0)
                .partial_cmp(&a.score.unwrap_or(0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        sorted.truncate(limit);
        return Ok(sorted);
    }

    output.status("Reranking", &format!("{} candidates", all_candidates.len()));

    let rerank_provider_config = rerank_config.to_provider_config(None);
    let rerank_provider = model_provider::create_rerank_provider(&rerank_provider_config)?;
    let documents: Vec<&str> = all_candidates.iter().map(|r| r.content.as_str()).collect();
    let reranked = rerank_provider
        .rerank(query, &documents, Some(limit))
        .await?;

    let results = reranked
        .iter()
        .filter_map(|item| {
            all_candidates.get(item.index).map(|result| {
                let mut r = result.clone();
                r.score = Some(item.score as f32);
                r.score_type = Some(ScoreType::Rerank);
                r
            })
        })
        .collect();

    Ok(results)
}

/// 候选数大于需求数且质量不足够高时才做 rerank
fn should_use_rerank(candidates: &[QueryResult], limit: usize) -> bool {
    if candidates.len() <= limit {
        return false;
    }

    let avg_score = {
        let scores: Vec<f32> = candidates.iter().filter_map(|c| c.score).collect();
        if scores.is_empty() {
            0.0
        } else {
            scores.iter().sum::<f32>() / scores.len() as f32
        }
    };

    match candidates.len() {
        1..=15 if avg_score > 0.80 => false,
        16..=25 if avg_score > 0.85 => false,
        _ => true,
    }
}

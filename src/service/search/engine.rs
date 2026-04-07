use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

use crate::ui::Output;
use lmkit::RerankProvider;
use memo_types::{QueryResult, ScoreType};

/// 对候选集执行 rerank 并返回最终结果
pub async fn apply_rerank(
    all_candidates: Vec<QueryResult>,
    query: &str,
    limit: usize,
    rerank: Arc<dyn RerankProvider>,
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

    let candidate_count = all_candidates.len();
    let t_rerank = Instant::now();

    let documents: Vec<&str> = all_candidates.iter().map(|r| r.content.as_str()).collect();
    let reranked = rerank.rerank(query, &documents, Some(limit)).await?;

    output.status_timed(
        "Reranked",
        &format!("{} candidates", candidate_count),
        t_rerank.elapsed(),
    );

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

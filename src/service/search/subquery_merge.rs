use std::collections::HashMap;

use crate::config::MergeConfig;

use super::types::{MergedResult, SubQueryResult};

/// 将多个子查询结果树形合并去重，返回最终结果列表
pub fn merge_results(sub_results: Vec<SubQueryResult>, config: &MergeConfig) -> Vec<MergedResult> {
    // 第一步：按 memory.id 去重，同一记忆保留最高分，记录所有来源
    let mut id_map: HashMap<String, MergedResult> = HashMap::new();

    for sub_result in &sub_results {
        let source_label = &sub_result.node_id;

        for memory in &sub_result.results {
            let score = memory.score.unwrap_or(0.0);
            let entry = id_map
                .entry(memory.id.clone())
                .or_insert_with(|| MergedResult {
                    memory: memory.clone(),
                    sources: Vec::new(),
                    max_score: score,
                });

            if score > entry.max_score {
                entry.max_score = score;
                entry.memory.score = Some(score);
                entry.memory.score_type = memory.score_type;
            }

            if !entry.sources.contains(source_label) {
                entry.sources.push(source_label.clone());
            }
        }
    }

    let mut merged: Vec<MergedResult> = id_map.into_values().collect();

    // 第二步：按最高分降序排序
    merged.sort_by(|a, b| {
        b.max_score
            .partial_cmp(&a.max_score)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // 第三步：确保每个叶子节点至少保留 min_per_leaf 条（暂不实现，保持简化）
    // TODO: 如果需要 min_per_leaf 功能，可以从旧代码恢复

    // 第四步：限制总数
    merged.truncate(config.max_results);

    tracing::debug!("Merge complete: {} final results", merged.len());

    merged
}

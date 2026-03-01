use std::collections::{HashMap, HashSet};

use crate::config::MultiQueryConfig;

use super::types::{MergedResult, SubQueryResult};

/// 将多个子查询结果树形合并去重，返回最终结果列表
pub fn merge_results(
    sub_results: Vec<SubQueryResult>,
    config: &MultiQueryConfig,
) -> Vec<MergedResult> {
    // 第一步：按 memory.id 去重，同一记忆保留最高分，记录所有来源
    let mut id_map: HashMap<String, MergedResult> = HashMap::new();

    for sub_result in &sub_results {
        let source_label = format!("{}:{}", sub_result.dimension.as_str(), sub_result.node_id);

        for memory in &sub_result.results {
            let score = memory.score.unwrap_or(0.0);
            let entry = id_map.entry(memory.id.clone()).or_insert_with(|| MergedResult {
                memory: memory.clone(),
                sources: Vec::new(),
                max_score: score,
            });

            if score > entry.max_score {
                entry.max_score = score;
                entry.memory.score = Some(score);
                entry.memory.score_type = memory.score_type.clone();
            }

            if !entry.sources.contains(&source_label) {
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

    // 第三步：确保每个叶子节点至少保留 min_per_leaf 条
    let guaranteed = collect_guaranteed_results(&merged, &sub_results, config.min_per_leaf);

    // 第四步：合并保证项和其他高分项，去重，限制总数
    let final_results = build_final_results(merged, guaranteed, config.max_total_results);

    tracing::debug!("Merge complete: {} final results", final_results.len());

    final_results
}

/// 收集每个叶子节点保证保留的记忆 ID 集合
fn collect_guaranteed_results(
    sorted_merged: &[MergedResult],
    sub_results: &[SubQueryResult],
    min_per_leaf: usize,
) -> HashSet<String> {
    let mut guaranteed_ids: HashSet<String> = HashSet::new();

    for sub_result in sub_results {
        let source_label = format!("{}:{}", sub_result.dimension.as_str(), sub_result.node_id);
        let mut count = 0;

        for merged in sorted_merged {
            if count >= min_per_leaf {
                break;
            }
            if merged.sources.contains(&source_label)
                && !guaranteed_ids.contains(&merged.memory.id)
            {
                guaranteed_ids.insert(merged.memory.id.clone());
                count += 1;
            }
        }
    }

    guaranteed_ids
}

/// 构建最终结果：优先保证项，再按分数填充，限制总数
fn build_final_results(
    sorted_merged: Vec<MergedResult>,
    guaranteed_ids: HashSet<String>,
    max_total: usize,
) -> Vec<MergedResult> {
    let mut seen_ids: HashSet<String> = HashSet::new();
    let mut results: Vec<MergedResult> = Vec::new();

    // 首先加入保证项（按分数顺序）
    for item in &sorted_merged {
        if guaranteed_ids.contains(&item.memory.id) && seen_ids.insert(item.memory.id.clone()) {
            results.push(item.clone());
            if results.len() >= max_total {
                return results;
            }
        }
    }

    // 再按分数顺序填充剩余名额
    for item in sorted_merged {
        if results.len() >= max_total {
            break;
        }
        if seen_ids.insert(item.memory.id.clone()) {
            results.push(item);
        }
    }

    results
}

use anyhow::Result;
use futures::future::join_all;

use crate::config::DecompositionConfig;
use crate::llm::{decompose_query, LlmClient, SubQuery as LlmSubQuery};

use super::types::{DecompositionTree, TreeNode};

/// BFS 递归拆解用户查询，构建拆解树
///
/// `strategy` 为用户自定义拆解策略段，传 `None` 使用内置五维策略。
pub async fn build_decomposition_tree(
    original_query: &str,
    llm_client: &LlmClient,
    config: &DecompositionConfig,
    strategy: Option<&str>,
) -> Result<DecompositionTree> {
    let mut tree = DecompositionTree::new();

    // 当前层待拆解的节点：(parent_id, query_text)
    let mut current_level_queue: Vec<(Option<String>, String)> =
        vec![(None, original_query.to_string())];

    let mut current_level = 0;

    while !current_level_queue.is_empty() {
        if current_level >= config.max_level {
            tracing::debug!("BFS: reached max_level={}", config.max_level);
            break;
        }

        if tree.leaf_count() >= config.max_total_leaves {
            tracing::debug!("BFS: reached max_total_leaves={}", config.max_total_leaves);
            break;
        }

        tracing::debug!(
            "BFS level {}: decomposing {} nodes",
            current_level,
            current_level_queue.len()
        );

        let decompose_tasks: Vec<_> = current_level_queue
            .iter()
            .map(|(_, query)| decompose_query(llm_client, query, strategy))
            .collect();

        let results = join_all(decompose_tasks).await;

        let mut next_level_queue: Vec<(Option<String>, String)> = Vec::new();

        for ((parent_id, _), result) in current_level_queue.iter().zip(results) {
            match result {
                Ok(subqueries) => {
                    let subqueries = limit_children(subqueries, config.max_children);
                    for sq in subqueries {
                        let node_id = tree.alloc_id();
                        let node = TreeNode {
                            id: node_id.clone(),
                            query: sq.question.clone(),
                            children: Vec::new(),
                        };
                        tree.add_node(node);

                        if let Some(pid) = parent_id {
                            if let Some(parent_node) = tree.nodes.get_mut(pid) {
                                parent_node.children.push(node_id.clone());
                            }
                        }

                        if sq.need_expand
                            && current_level + 1 < config.max_level
                            && tree.leaf_count() < config.max_total_leaves
                        {
                            next_level_queue.push((Some(node_id), sq.question));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("BFS level {} decompose failed: {}", current_level, e);
                }
            }
        }

        current_level_queue = next_level_queue;
        current_level += 1;
    }

    tracing::debug!(
        "BFS complete: {} total nodes, {} leaves",
        tree.node_count(),
        tree.leaf_count()
    );

    Ok(tree)
}

fn limit_children(subqueries: Vec<LlmSubQuery>, max_children: usize) -> Vec<LlmSubQuery> {
    subqueries.into_iter().take(max_children).collect()
}

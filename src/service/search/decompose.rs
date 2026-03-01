use anyhow::Result;
use futures::future::join_all;

use crate::config::DecompositionConfig;
use crate::llm::{decompose_query, LlmClient, SubQuery};

use super::types::{DecompositionTree, SearchDimension, TreeNode};

/// BFS 递归拆解用户查询，构建拆解树
pub async fn build_decomposition_tree(
    original_query: &str,
    llm_client: &LlmClient,
    config: &DecompositionConfig,
) -> Result<DecompositionTree> {
    let mut tree = DecompositionTree::new();

    // 当前层待拆解的节点：(node_id, query_text, level)
    let mut current_level_queue: Vec<(Option<String>, String, usize)> = vec![(
        None,
        original_query.to_string(),
        0,
    )];

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

        // 并行拆解当前层所有节点
        let decompose_tasks: Vec<_> = current_level_queue
            .iter()
            .map(|(_, query, _)| decompose_query(llm_client, query))
            .collect();

        let results = join_all(decompose_tasks).await;

        let mut next_level_queue: Vec<(Option<String>, String, usize)> = Vec::new();

        for ((parent_id, _, level), result) in current_level_queue.iter().zip(results) {
            match result {
                Ok(subqueries) => {
                    let subqueries = limit_children(subqueries, config.max_children);
                    for sq in subqueries {
                        let node_id = tree.alloc_id();
                        let node = TreeNode {
                            id: node_id.clone(),
                            dimension: SearchDimension::from_str(&sq.dimension),
                            query: sq.query.clone(),
                            children: Vec::new(),
                        };
                        tree.add_node(node);

                        if let Some(pid) = parent_id {
                            if let Some(parent_node) = tree.nodes.get_mut(pid) {
                                parent_node.children.push(node_id.clone());
                            }
                        }

                        if sq.needs_refinement
                            && level + 1 < config.max_level
                            && tree.leaf_count() < config.max_total_leaves
                        {
                            next_level_queue.push((Some(node_id), sq.query, level + 1));
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "BFS level {} decompose failed: {}",
                        current_level,
                        e
                    );
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

fn limit_children(subqueries: Vec<SubQuery>, max_children: usize) -> Vec<SubQuery> {
    subqueries.into_iter().take(max_children).collect()
}

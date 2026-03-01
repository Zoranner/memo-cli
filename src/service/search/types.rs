use std::collections::HashMap;

use memo_types::QueryResult;

/// 拆解树节点
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub query: String,
    pub children: Vec<String>,
}

/// 拆解树
#[derive(Debug, Default)]
pub struct DecompositionTree {
    pub nodes: HashMap<String, TreeNode>,
    id_counter: usize,
}

impl DecompositionTree {
    pub fn new() -> Self {
        Self::default()
    }

    /// 分配唯一节点 ID
    pub fn alloc_id(&mut self) -> String {
        let id = format!("node_{}", self.id_counter);
        self.id_counter += 1;
        id
    }

    /// 添加节点
    pub fn add_node(&mut self, node: TreeNode) {
        self.nodes.insert(node.id.clone(), node);
    }

    /// 获取所有叶子节点（无子节点的节点）
    pub fn get_leaves(&self) -> Vec<&TreeNode> {
        self.nodes
            .values()
            .filter(|n| n.children.is_empty())
            .collect()
    }

    /// 获取节点总数
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// 获取叶子节点数量
    pub fn leaf_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| n.children.is_empty())
            .count()
    }
}

/// 子查询搜索结果（单个叶子节点的结果）
#[derive(Debug, Clone)]
pub struct SubQueryResult {
    pub node_id: String,
    pub results: Vec<QueryResult>,
}

/// 合并后的最终结果
#[derive(Debug, Clone)]
pub struct MergedResult {
    pub memory: QueryResult,
    /// 来源：来自哪些叶子节点（node_id）
    pub sources: Vec<String>,
    pub max_score: f32,
}

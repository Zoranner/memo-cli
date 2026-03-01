use std::collections::HashMap;

use memo_types::QueryResult;

/// 五维搜索维度
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SearchDimension {
    Core,
    Why,
    How,
    Case,
    Note,
    Unknown(String),
}

impl SearchDimension {
    pub fn from_str(s: &str) -> Self {
        match s {
            "core" => Self::Core,
            "why" => Self::Why,
            "how" => Self::How,
            "case" => Self::Case,
            "note" => Self::Note,
            other => Self::Unknown(other.to_string()),
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::Core => "core",
            Self::Why => "why",
            Self::How => "how",
            Self::Case => "case",
            Self::Note => "note",
            Self::Unknown(s) => s.as_str(),
        }
    }
}

/// 拆解树节点
#[derive(Debug, Clone)]
pub struct TreeNode {
    pub id: String,
    pub dimension: SearchDimension,
    pub query: String,
    pub children: Vec<String>,
}

/// 拆解树
#[derive(Debug)]
pub struct DecompositionTree {
    pub nodes: HashMap<String, TreeNode>,
    id_counter: usize,
}

impl DecompositionTree {
    pub fn new() -> Self {
        Self {
            nodes: HashMap::new(),
            id_counter: 0,
        }
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
        self.nodes.values().filter(|n| n.children.is_empty()).count()
    }
}

/// 子查询搜索结果（单个叶子节点的结果）
#[derive(Debug, Clone)]
pub struct SubQueryResult {
    pub node_id: String,
    pub dimension: SearchDimension,
    pub results: Vec<QueryResult>,
}

/// 合并后的最终结果
#[derive(Debug, Clone)]
pub struct MergedResult {
    pub memory: QueryResult,
    /// 来源：来自哪些叶子节点（格式：维度:node_id）
    pub sources: Vec<String>,
    pub max_score: f32,
}

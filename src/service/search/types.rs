use memo_types::QueryResult;

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
    /// 来自哪些叶子节点（node_id）
    pub sources: Vec<String>,
    pub max_score: f32,
}

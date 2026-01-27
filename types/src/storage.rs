use anyhow::Result;
use async_trait::async_trait;

use crate::models::{Memory, QueryResult, TimeRange};

/// 存储后端的统一接口
///
/// 任何存储实现（本地、远程、云端）都应该实现这个 trait
#[async_trait]
pub trait StorageBackend: Send + Sync {
    /// 连接/初始化存储
    async fn connect(config: &StorageConfig) -> Result<Self>
    where
        Self: Sized;

    /// 获取向量维度
    fn dimension(&self) -> usize;

    /// 检查表/集合是否存在
    async fn exists(&self) -> Result<bool>;

    /// 初始化表/集合
    async fn init(&self) -> Result<()>;

    /// 获取记录总数
    async fn count(&self) -> Result<usize>;

    /// 插入记忆（vector 由外部生成）
    async fn insert(&self, memory: Memory) -> Result<()>;

    /// 批量插入
    async fn insert_batch(&self, memories: Vec<Memory>) -> Result<()>;

    /// 向量搜索（接收已生成的向量）
    async fn search_by_vector(
        &self,
        vector: Vec<f32>,
        limit: usize,
        threshold: f32,
        time_range: Option<TimeRange>,
    ) -> Result<Vec<QueryResult>>;

    /// 列出所有记忆
    async fn list(&self) -> Result<Vec<QueryResult>>;

    /// 根据 ID 查找
    async fn find_by_id(&self, id: &str) -> Result<Option<QueryResult>>;

    /// 根据 ID 查找完整的 Memory（包括 vector 和 created_at）
    /// 用于需要访问完整数据的内部操作（如 update、merge）
    async fn find_memory_by_id(&self, id: &str) -> Result<Option<Memory>>;

    /// 查找相似记忆（接收向量）
    async fn find_similar(
        &self,
        vector: Vec<f32>,
        limit: usize,
        threshold: f32,
        exclude_id: Option<&str>,
    ) -> Result<Vec<QueryResult>>;

    /// 更新记忆（需要重新生成向量）
    async fn update(
        &self,
        id: &str,
        content: String,
        vector: Vec<f32>,
        tags: Vec<String>,
    ) -> Result<()>;

    /// 删除记忆
    async fn delete(&self, id: &str) -> Result<()>;

    /// 清空所有记忆
    async fn clear(&self) -> Result<()>;
}

/// 存储配置（通用）
#[derive(Debug, Clone)]
pub struct StorageConfig {
    pub path: String,
    pub dimension: usize,
    // 未来可扩展：url, auth, etc.
}

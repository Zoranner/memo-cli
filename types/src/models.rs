use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// 核心记忆数据结构（完全独立，不依赖任何数据库）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub vector: Vec<f32>,
    pub source_file: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// 查询结果（用于返回搜索/列表结果）
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QueryResult {
    pub id: String,
    pub content: String,
    pub tags: Vec<String>,
    pub updated_at: i64,
    pub score: Option<f32>,
}

/// 时间范围过滤
#[derive(Debug, Clone)]
pub struct TimeRange {
    pub after: Option<i64>,
    pub before: Option<i64>,
}

/// 用于构建 Memory 的 Builder
pub struct MemoryBuilder {
    pub content: String,
    pub tags: Vec<String>,
    pub vector: Vec<f32>,
    pub source_file: Option<String>,
}

impl Memory {
    pub fn new(builder: MemoryBuilder) -> Self {
        use uuid::Uuid;
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            content: builder.content,
            tags: builder.tags,
            vector: builder.vector,
            source_file: builder.source_file,
            created_at: now,
            updated_at: now,
        }
    }
}

/// Markdown 解析相关（非数据库）
#[derive(Debug, Clone)]
pub struct MemoSection {
    pub content: String,
    pub metadata: MemoMetadata,
}

#[derive(Debug, Clone)]
pub struct MemoMetadata {
    pub tags: Vec<String>,
}

use anyhow::{Context, Result};
use arrow_array::RecordBatch;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

/// 查询结果
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub id: String,
    pub content: String,
    pub tags: Option<Vec<String>>,
    pub updated_at: i64,
    pub score: Option<f32>,
}

/// 查询类型
#[derive(Clone)]
pub enum QueryType {
    /// 向量搜索
    VectorSearch(Vec<f32>),
    /// 普通列表查询
    List,
}

/// 查询构建器
pub struct QueryBuilder<'a> {
    table: &'a lancedb::Table,
    query_type: QueryType,
    columns: Vec<&'static str>,
    limit: Option<usize>,
    threshold: Option<f32>,
    after: Option<i64>,
    before: Option<i64>,
}

impl<'a> QueryBuilder<'a> {
    /// 创建一个新的查询构建器
    pub fn new(table: &'a lancedb::Table, query_type: QueryType) -> Self {
        Self {
            table,
            query_type,
            columns: vec!["id", "content", "updated_at"],
            limit: None,
            threshold: None,
            after: None,
            before: None,
        }
    }

    /// 创建向量搜索查询
    pub fn vector_search(table: &'a lancedb::Table, embedding: Vec<f32>) -> Self {
        Self::new(table, QueryType::VectorSearch(embedding))
    }

    /// 创建列表查询
    pub fn list(table: &'a lancedb::Table) -> Self {
        Self::new(table, QueryType::List)
    }

    /// 选择要查询的列
    pub fn select_columns(mut self, columns: Vec<&'static str>) -> Self {
        self.columns = columns;
        self
    }

    /// 设置返回结果的最大数量
    pub fn limit(mut self, limit: usize) -> Self {
        self.limit = Some(limit);
        self
    }

    /// 设置相似度阈值（仅用于向量搜索）
    pub fn threshold(mut self, threshold: f32) -> Self {
        self.threshold = Some(threshold);
        self
    }

    /// 设置时间范围过滤
    pub fn time_range(mut self, after: Option<i64>, before: Option<i64>) -> Self {
        self.after = after;
        self.before = before;
        self
    }

    /// 执行查询并返回结果
    pub async fn execute(self) -> Result<Vec<QueryResult>> {
        match self.query_type.clone() {
            QueryType::VectorSearch(embedding) => self.execute_vector_search(embedding).await,
            QueryType::List => self.execute_list().await,
        }
    }

    /// 执行向量搜索查询
    async fn execute_vector_search(self, embedding: Vec<f32>) -> Result<Vec<QueryResult>> {
        // 如果有时间过滤，增加查询限制以便后续过滤
        let query_limit = if self.after.is_some() || self.before.is_some() {
            self.limit.unwrap_or(100) * 10
        } else {
            self.limit.unwrap_or(100)
        };

        let mut stream = self
            .table
            .vector_search(embedding)?
            .select(lancedb::query::Select::columns(&self.columns))
            .limit(query_limit)
            .execute()
            .await?;

        let mut results = Vec::new();

        while let Some(batch) = stream.try_next().await? {
            let batch_results = self.process_batch(&batch, true)?;
            results.extend(batch_results);
        }

        // 限制结果数量
        if let Some(limit) = self.limit {
            results.truncate(limit);
        }

        Ok(results)
    }

    /// 执行列表查询
    async fn execute_list(self) -> Result<Vec<QueryResult>> {
        let mut stream = self
            .table
            .query()
            .select(lancedb::query::Select::columns(&self.columns))
            .execute()
            .await?;

        let mut results = Vec::new();

        while let Some(batch) = stream.try_next().await? {
            let batch_results = self.process_batch(&batch, false)?;
            results.extend(batch_results);
        }

        Ok(results)
    }

    /// 处理单个 RecordBatch
    fn process_batch(&self, batch: &RecordBatch, has_distance: bool) -> Result<Vec<QueryResult>> {
        let ids = extract_string_column(batch, "id")?;
        let contents = extract_string_column(batch, "content")?;
        let updated_ats = extract_timestamp_column(batch, "updated_at")?;

        // 可选的列
        let tags_col = if self.columns.contains(&"tags") {
            Some(extract_string_column(batch, "tags")?)
        } else {
            None
        };

        let distances = if has_distance && self.columns.contains(&"_distance") {
            Some(extract_float_column(batch, "_distance")?)
        } else {
            None
        };

        let mut results = Vec::new();

        for i in 0..batch.num_rows() {
            let timestamp = updated_ats.value(i);

            // 时间范围过滤
            if let Some(after) = self.after {
                if timestamp < after {
                    continue;
                }
            }

            if let Some(before) = self.before {
                if timestamp > before {
                    continue;
                }
            }

            // 计算相似度分数
            let score = if let Some(distances) = distances {
                let distance = distances.value(i);
                let score = calculate_similarity_score(distance);

                // 阈值过滤
                if let Some(threshold) = self.threshold {
                    if score < threshold {
                        continue;
                    }
                }

                Some(score)
            } else {
                None
            };

            // 解析 tags
            let tags = if let Some(tags_col) = tags_col {
                let tags_json = tags_col.value(i);
                serde_json::from_str(tags_json).ok()
            } else {
                None
            };

            results.push(QueryResult {
                id: ids.value(i).to_string(),
                content: contents.value(i).to_string(),
                tags,
                updated_at: timestamp,
                score,
            });
        }

        Ok(results)
    }
}

/// 计算相似度分数（从 L2 距离转换）
pub fn calculate_similarity_score(distance: f32) -> f32 {
    (1.0 - (distance / 2.0)).max(0.0)
}

/// 从 RecordBatch 中提取字符串列
pub fn extract_string_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a arrow_array::StringArray> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
        .with_context(|| format!("Failed to get {} column", name))
}

/// 从 RecordBatch 中提取时间戳列
pub fn extract_timestamp_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a arrow_array::TimestampMillisecondArray> {
    batch
        .column_by_name(name)
        .and_then(|c| {
            c.as_any()
                .downcast_ref::<arrow_array::TimestampMillisecondArray>()
        })
        .with_context(|| format!("Failed to get {} column", name))
}

/// 从 RecordBatch 中提取浮点数列
pub fn extract_float_column<'a>(
    batch: &'a RecordBatch,
    name: &str,
) -> Result<&'a arrow_array::Float32Array> {
    batch
        .column_by_name(name)
        .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>())
        .with_context(|| format!("Failed to get {} column", name))
}

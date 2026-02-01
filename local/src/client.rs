use anyhow::{Context, Result};
use arrow_array::{
    Array, ArrayRef, RecordBatch, RecordBatchIterator, StringArray, TimestampMillisecondArray,
};
use async_trait::async_trait;
use chrono::Utc;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase, Select};
use std::sync::Arc;

use memo_types::{Memory, QueryResult, StorageBackend, StorageConfig, TimeRange};

use crate::db::{Connection, DatabaseMetadata, TableOperations};

/// LanceDB 本地存储客户端
pub struct LocalStorageClient {
    conn: Connection,
    dimension: usize,
}

#[async_trait]
impl StorageBackend for LocalStorageClient {
    async fn connect(config: &StorageConfig) -> Result<Self> {
        let path = std::path::Path::new(&config.path);
        let conn = Connection::connect(path).await?;

        // 如果数据库已存在，加载并验证元数据
        if TableOperations::table_exists(conn.inner(), "memories").await {
            let metadata = DatabaseMetadata::load(path)?;
            metadata.validate_dimension(config.dimension)?;
        }

        Ok(Self {
            conn,
            dimension: config.dimension,
        })
    }

    fn dimension(&self) -> usize {
        self.dimension
    }

    async fn exists(&self) -> Result<bool> {
        Ok(self.table_exists().await)
    }

    async fn init(&self) -> Result<()> {
        self.init_table().await?;
        Ok(())
    }

    async fn count(&self) -> Result<usize> {
        if !self.table_exists().await {
            return Ok(0);
        }
        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;
        Ok(table.count_rows(None).await.unwrap_or(0))
    }

    async fn insert(&self, memory: Memory) -> Result<()> {
        // 验证向量维度
        if memory.vector.len() != self.dimension {
            anyhow::bail!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                memory.vector.len()
            );
        }

        self.init_table().await?;
        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;

        // 转换为 Arrow RecordBatch
        let batch = memory_to_record_batch(&memory)?;
        let schema = batch.schema();
        let batches = RecordBatchIterator::new(vec![Ok(batch)], schema);

        table.add(Box::new(batches)).execute().await?;
        Ok(())
    }

    async fn insert_batch(&self, memories: Vec<Memory>) -> Result<()> {
        // 验证所有向量维度
        for memory in &memories {
            if memory.vector.len() != self.dimension {
                anyhow::bail!(
                    "Vector dimension mismatch: expected {}, got {}",
                    self.dimension,
                    memory.vector.len()
                );
            }
        }

        self.init_table().await?;
        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;

        // 转换所有 Memory 为 RecordBatch
        let mut batches = Vec::new();
        for memory in &memories {
            batches.push(Ok(memory_to_record_batch(memory)?));
        }

        let schema = batches
            .first()
            .and_then(|b| b.as_ref().ok().map(|batch| batch.schema()))
            .context("No batches to insert")?;

        let batch_iter = RecordBatchIterator::new(batches, schema);
        table.add(Box::new(batch_iter)).execute().await?;
        Ok(())
    }

    async fn search_by_vector(
        &self,
        vector: Vec<f32>,
        limit: usize,
        threshold: f32,
        time_range: Option<TimeRange>,
    ) -> Result<Vec<QueryResult>> {
        // 验证维度
        if vector.len() != self.dimension {
            anyhow::bail!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            );
        }

        if !self.table_exists().await {
            return Ok(vec![]);
        }

        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;

        let mut query = table
            .vector_search(vector)?
            .select(Select::columns(&[
                "id",
                "content",
                "tags",
                "updated_at",
                "_distance",
            ]))
            .limit(limit);

        // 添加时间过滤
        if let Some(range) = time_range {
            if let Some(after) = range.after {
                query = query.only_if(format!("updated_at >= {}", after));
            }
            if let Some(before) = range.before {
                query = query.only_if(format!("updated_at <= {}", before));
            }
        }

        let mut stream = query.execute().await?;
        let mut batches = Vec::new();
        while let Some(batch) = stream.try_next().await? {
            batches.push(batch);
        }

        parse_query_results(batches, Some(threshold))
    }

    async fn list(&self) -> Result<Vec<QueryResult>> {
        if !self.table_exists().await {
            return Ok(vec![]);
        }

        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;

        let query = table
            .query()
            .select(Select::columns(&["id", "content", "tags", "updated_at"]))
            .only_if("id IS NOT NULL");

        let mut stream = query.execute().await?;
        let mut batches = Vec::new();
        while let Some(batch) = stream.try_next().await? {
            batches.push(batch);
        }

        parse_query_results(batches, None)
    }

    async fn find_by_id(&self, id: &str) -> Result<Option<QueryResult>> {
        if !self.table_exists().await {
            return Ok(None);
        }

        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;

        let query = table
            .query()
            .select(Select::columns(&["id", "content", "tags", "updated_at"]))
            .only_if(format!("id = '{}'", id));

        let mut stream = query.execute().await?;
        let mut batches = Vec::new();
        while let Some(batch) = stream.try_next().await? {
            batches.push(batch);
        }

        let results = parse_query_results(batches, None)?;
        Ok(results.into_iter().next())
    }

    async fn find_memory_by_id(&self, id: &str) -> Result<Option<Memory>> {
        self.find_memory_by_id_impl(id).await
    }

    async fn find_similar(
        &self,
        vector: Vec<f32>,
        limit: usize,
        threshold: f32,
        exclude_id: Option<&str>,
    ) -> Result<Vec<QueryResult>> {
        let results = self
            .search_by_vector(vector, limit, threshold, None)
            .await?;

        // 过滤掉指定 ID
        if let Some(eid) = exclude_id {
            Ok(results.into_iter().filter(|r| r.id != eid).collect())
        } else {
            Ok(results)
        }
    }

    async fn update(
        &self,
        id: &str,
        content: String,
        vector: Vec<f32>,
        tags: Vec<String>,
    ) -> Result<()> {
        if vector.len() != self.dimension {
            anyhow::bail!(
                "Vector dimension mismatch: expected {}, got {}",
                self.dimension,
                vector.len()
            );
        }

        // 查询旧记忆获取 created_at
        let old_memory = self
            .find_memory_by_id_impl(id)
            .await?
            .with_context(|| format!("Memory not found: {}", id))?;

        // 创建更新后的记忆（保留 created_at）
        let mut memory = Memory::new(memo_types::MemoryBuilder {
            content,
            tags,
            vector,
            source_file: None,
        });
        memory.id = id.to_string();
        memory.created_at = old_memory.created_at; // 保留创建时间
        memory.updated_at = Utc::now();

        // 先尝试插入新记忆
        // 注意：LanceDB 允许重复 ID，所以需要先删后插
        // 但我们将插入逻辑包装在错误处理中，失败时尝试恢复
        let insert_result = async {
            self.delete(id).await?;
            self.insert(memory.clone()).await
        }
        .await;

        // 如果插入失败，尝试恢复旧记忆
        if let Err(e) = insert_result {
            // 尝试恢复旧记忆
            if let Err(restore_err) = self.insert(old_memory).await {
                anyhow::bail!(
                    "Update failed and restore failed: update error: {}, restore error: {}",
                    e,
                    restore_err
                );
            }
            return Err(e);
        }

        Ok(())
    }

    async fn delete(&self, id: &str) -> Result<()> {
        if !self.table_exists().await {
            anyhow::bail!("Table does not exist");
        }

        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;
        table.delete(&format!("id = '{}'", id)).await?;

        Ok(())
    }

    async fn clear(&self) -> Result<()> {
        if !self.table_exists().await {
            return Ok(());
        }

        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;
        table.delete("id IS NOT NULL").await?;

        Ok(())
    }
}

// 私有辅助方法
impl LocalStorageClient {
    async fn table_exists(&self) -> bool {
        TableOperations::table_exists(self.conn.inner(), "memories").await
    }

    async fn init_table(&self) -> Result<()> {
        if !self.table_exists().await {
            TableOperations::create_table(self.conn.inner(), "memories", self.dimension).await?;
        }
        Ok(())
    }

    /// 查询完整的 Memory 对象（包括 vector 和 created_at）- 私有实现
    async fn find_memory_by_id_impl(&self, id: &str) -> Result<Option<Memory>> {
        if !self.table_exists().await {
            return Ok(None);
        }

        let table = TableOperations::open_table(self.conn.inner(), "memories").await?;

        let query = table
            .query()
            .select(Select::columns(&[
                "id",
                "content",
                "tags",
                "vector",
                "source_file",
                "created_at",
                "updated_at",
            ]))
            .only_if(format!("id = '{}'", id));

        let mut stream = query.execute().await?;
        let mut batches = Vec::new();
        while let Some(batch) = stream.try_next().await? {
            batches.push(batch);
        }

        // 解析完整的 Memory 对象
        for batch in batches {
            if batch.num_rows() == 0 {
                continue;
            }

            let id_array = batch
                .column_by_name("id")
                .context("Missing 'id' column")?
                .as_any()
                .downcast_ref::<StringArray>()
                .context("Invalid 'id' column type")?;

            let content_array = batch
                .column_by_name("content")
                .context("Missing 'content' column")?
                .as_any()
                .downcast_ref::<StringArray>()
                .context("Invalid 'content' column type")?;

            let tags_array = batch
                .column_by_name("tags")
                .context("Missing 'tags' column")?
                .as_any()
                .downcast_ref::<StringArray>()
                .context("Invalid 'tags' column type")?;

            let vector_array = batch
                .column_by_name("vector")
                .context("Missing 'vector' column")?
                .as_any()
                .downcast_ref::<arrow_array::FixedSizeListArray>()
                .context("Invalid 'vector' column type")?;

            let source_file_array = batch
                .column_by_name("source_file")
                .context("Missing 'source_file' column")?
                .as_any()
                .downcast_ref::<StringArray>()
                .context("Invalid 'source_file' column type")?;

            let created_at_array = batch
                .column_by_name("created_at")
                .context("Missing 'created_at' column")?
                .as_any()
                .downcast_ref::<TimestampMillisecondArray>()
                .context("Invalid 'created_at' column type")?;

            let updated_at_array = batch
                .column_by_name("updated_at")
                .context("Missing 'updated_at' column")?
                .as_any()
                .downcast_ref::<TimestampMillisecondArray>()
                .context("Invalid 'updated_at' column type")?;

            // 只取第一条记录
            if batch.num_rows() > 0 {
                let tags_str = tags_array.value(0);
                let tags: Vec<String> = serde_json::from_str(tags_str).unwrap_or_default();

                // 提取向量数据
                let vector_values = vector_array.value(0);
                let float_array = vector_values
                    .as_any()
                    .downcast_ref::<arrow_array::Float32Array>()
                    .context("Invalid vector float array")?;
                let vector: Vec<f32> = (0..float_array.len())
                    .map(|i| float_array.value(i))
                    .collect();

                let source_file = if source_file_array.is_null(0) {
                    None
                } else {
                    Some(source_file_array.value(0).to_string())
                };

                let created_at = chrono::DateTime::from_timestamp_millis(created_at_array.value(0))
                    .context("Invalid created_at timestamp")?;

                let updated_at = chrono::DateTime::from_timestamp_millis(updated_at_array.value(0))
                    .context("Invalid updated_at timestamp")?;

                return Ok(Some(Memory {
                    id: id_array.value(0).to_string(),
                    content: content_array.value(0).to_string(),
                    tags,
                    vector,
                    source_file,
                    created_at,
                    updated_at,
                }));
            }
        }

        Ok(None)
    }
}

/// 将 Memory 转换为 RecordBatch
fn memory_to_record_batch(memory: &Memory) -> Result<RecordBatch> {
    use arrow_array::{FixedSizeListArray, Float32Array};

    let schema = memory_schema(memory.vector.len());

    let id_array = StringArray::from(vec![memory.id.as_str()]);
    let content_array = StringArray::from(vec![memory.content.as_str()]);
    let tags_array = StringArray::from(vec![serde_json::to_string(&memory.tags)?]);

    let vector_values = Float32Array::from(memory.vector.clone());
    let vector_array = FixedSizeListArray::new(
        Arc::new(arrow_schema::Field::new(
            "item",
            arrow_schema::DataType::Float32,
            true,
        )),
        memory.vector.len() as i32,
        Arc::new(vector_values),
        None,
    );

    let source_file_array = StringArray::from(vec![memory.source_file.as_deref()]);
    let created_at_array =
        TimestampMillisecondArray::from(vec![memory.created_at.timestamp_millis()]);
    let updated_at_array =
        TimestampMillisecondArray::from(vec![memory.updated_at.timestamp_millis()]);

    let arrays: Vec<ArrayRef> = vec![
        Arc::new(id_array),
        Arc::new(content_array),
        Arc::new(tags_array),
        Arc::new(vector_array),
        Arc::new(source_file_array),
        Arc::new(created_at_array),
        Arc::new(updated_at_array),
    ];

    Ok(RecordBatch::try_new(schema, arrays)?)
}

/// 创建 memory schema
fn memory_schema(vector_dim: usize) -> Arc<arrow_schema::Schema> {
    use arrow_schema::{DataType, Field, Schema};

    Arc::new(Schema::new(vec![
        Field::new("id", DataType::Utf8, false),
        Field::new("content", DataType::Utf8, false),
        Field::new("tags", DataType::Utf8, false),
        Field::new(
            "vector",
            DataType::FixedSizeList(
                Arc::new(Field::new("item", DataType::Float32, true)),
                vector_dim as i32,
            ),
            false,
        ),
        Field::new("source_file", DataType::Utf8, true),
        Field::new(
            "created_at",
            DataType::Timestamp(arrow_schema::TimeUnit::Millisecond, None),
            false,
        ),
        Field::new(
            "updated_at",
            DataType::Timestamp(arrow_schema::TimeUnit::Millisecond, None),
            false,
        ),
    ]))
}

/// 解析查询结果
fn parse_query_results(
    batches: Vec<RecordBatch>,
    threshold: Option<f32>,
) -> Result<Vec<QueryResult>> {
    let mut results = Vec::new();

    for batch in batches {
        let num_rows = batch.num_rows();
        if num_rows == 0 {
            continue;
        }

        let id_array = batch
            .column_by_name("id")
            .context("Missing 'id' column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Invalid 'id' column type")?;

        let content_array = batch
            .column_by_name("content")
            .context("Missing 'content' column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Invalid 'content' column type")?;

        let tags_array = batch
            .column_by_name("tags")
            .context("Missing 'tags' column")?
            .as_any()
            .downcast_ref::<StringArray>()
            .context("Invalid 'tags' column type")?;

        let updated_at_array = batch
            .column_by_name("updated_at")
            .context("Missing 'updated_at' column")?
            .as_any()
            .downcast_ref::<TimestampMillisecondArray>()
            .context("Invalid 'updated_at' column type")?;

        let distance_array = batch
            .column_by_name("_distance")
            .and_then(|col| col.as_any().downcast_ref::<arrow_array::Float32Array>());

        for i in 0..num_rows {
            let score = distance_array
                .filter(|arr| arr.is_valid(i))
                .map(|arr| 1.0 - arr.value(i));

            // 应用阈值过滤
            if let (Some(s), Some(t)) = (score, threshold) {
                if s < t {
                    continue;
                }
            }

            let tags_str = tags_array.value(i);
            let tags: Vec<String> = serde_json::from_str(tags_str).unwrap_or_default();

            results.push(QueryResult {
                id: id_array.value(i).to_string(),
                content: content_array.value(i).to_string(),
                tags,
                updated_at: updated_at_array.value(i),
                score,
                score_type: Some(memo_types::ScoreType::Vector),
            });
        }
    }

    Ok(results)
}

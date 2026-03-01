use anyhow::Result;
use lancedb::table::Table;
use std::sync::Arc;

pub struct TableOperations;

impl TableOperations {
    pub async fn create_table(
        conn: &lancedb::connection::Connection,
        table_name: &str,
        vector_dim: usize,
    ) -> Result<Table> {
        use arrow_schema::{DataType, Field, Schema};

        let schema = Arc::new(Schema::new(vec![
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
        ]));

        let table = conn
            .create_empty_table(table_name, schema)
            .execute()
            .await?;

        Ok(table)
    }

    pub async fn open_table(
        conn: &lancedb::connection::Connection,
        table_name: &str,
    ) -> Result<Table> {
        let table = conn.open_table(table_name).execute().await?;

        Ok(table)
    }

    pub async fn table_exists(conn: &lancedb::connection::Connection, table_name: &str) -> bool {
        conn.table_names()
            .execute()
            .await
            .map(|names| names.contains(&table_name.to_string()))
            .unwrap_or(false)
    }
}

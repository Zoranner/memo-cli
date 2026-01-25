use anyhow::Result;
use lancedb::table::Table;

use crate::models::memory_schema;

pub struct TableOperations;

impl TableOperations {
    pub async fn create_table(
        conn: &lancedb::connection::Connection,
        table_name: &str,
    ) -> Result<Table> {
        let schema = memory_schema();

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

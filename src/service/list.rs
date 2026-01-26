use anyhow::{Context, Result};
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

use crate::config::Config;
use crate::db::{Connection, TableOperations};
use crate::ui::Output;

pub async fn list(force_local: bool, force_global: bool) -> Result<()> {
    let output = Output::new();

    // 自动初始化
    let _initialized = crate::service::init::ensure_initialized().await?;

    let config = Config::load_with_scope(force_local, force_global)?;

    let conn = Connection::connect(&config.brain_path).await?;
    let table = TableOperations::open_table(conn.inner(), "memories").await?;
    let record_count = table.count_rows(None).await.unwrap_or(0);

    // 显示数据库信息
    output.database_info(&config.brain_path, record_count);

    if record_count == 0 {
        output.info("No memories found. Use 'memo embed' to add some!");
        return Ok(());
    }

    // 只查询需要的列，提升性能
    let mut stream = table
        .query()
        .select(lancedb::query::Select::columns(&[
            "id",
            "content",
            "updated_at",
        ]))
        .execute()
        .await?;

    let mut total_count = 0;

    while let Some(batch) = stream.try_next().await? {
        let ids = batch
            .column_by_name("id")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            .context("Failed to get id column")?;

        let contents = batch
            .column_by_name("content")
            .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
            .context("Failed to get content column")?;

        let updated_ats = batch
            .column_by_name("updated_at")
            .and_then(|c| {
                c.as_any()
                    .downcast_ref::<arrow_array::TimestampMillisecondArray>()
            })
            .context("Failed to get updated_at column")?;

        for i in 0..batch.num_rows() {
            total_count += 1;
            let updated = updated_ats
                .value_as_datetime(i)
                .map(|ts| ts.format("%Y-%m-%d %H:%M").to_string())
                .unwrap_or_else(|| "N/A".to_string());

            output.list_item(
                total_count,
                record_count,
                ids.value(i),
                &updated,
                contents.value(i),
            );

            // 如果不是最后一个结果，添加空行分隔
            if i < batch.num_rows() - 1 {
                println!();
            }
        }
    }

    Ok(())
}

use anyhow::Result;

use crate::config::Config;
use crate::db::{Connection, TableOperations};
use crate::service::query::QueryBuilder;
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

    // 使用通用查询构建器
    let results = QueryBuilder::list(&table)
        .select_columns(vec!["id", "content", "tags", "updated_at"])
        .execute()
        .await?;

    // 显示结果
    output.list_items(&results);

    Ok(())
}

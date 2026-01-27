use anyhow::{Context, Result};
use chrono::NaiveDateTime;

use crate::config::Config;
use crate::db::{Connection, TableOperations};
use crate::embedding::EmbeddingModel;
use crate::service::query::QueryBuilder;
use crate::ui::Output;

pub async fn search(
    query: &str,
    limit: usize,
    threshold: f32,
    after: Option<String>,
    before: Option<String>,
    force_local: bool,
    force_global: bool,
) -> Result<()> {
    let output = Output::new();

    // 自动初始化
    let _initialized = crate::service::init::ensure_initialized().await?;

    let config = Config::load_with_scope(force_local, force_global)?;

    // 连接数据库并显示基本信息
    let conn = Connection::connect(&config.brain_path).await?;
    let table = TableOperations::open_table(conn.inner(), "memories").await?;
    let record_count = table.count_rows(None).await.unwrap_or(0);

    // 检查 API key（Ollama 不需要）
    config.validate_api_key(force_local)?;

    // 显示数据库信息
    output.database_info(&config.brain_path, record_count);

    let model = EmbeddingModel::new(
        config.embedding_api_key.clone(),
        config.embedding_model.clone(),
        config.embedding_base_url.clone(),
        config.embedding_dimension,
        config.embedding_provider.clone(),
    )?;

    output.status("Encoding", "query");
    let query_vector = model.encode(query).await?;

    output.status("Searching", "database");

    // 解析时间过滤参数
    let after_ts = if let Some(after_str) = &after {
        Some(parse_datetime(after_str)?)
    } else {
        None
    };

    let before_ts = if let Some(before_str) = &before {
        Some(parse_datetime(before_str)?)
    } else {
        None
    };

    // 使用通用查询构建器
    let results = QueryBuilder::vector_search(&table, query_vector)
        .select_columns(vec!["id", "content", "tags", "updated_at", "_distance"])
        .limit(limit)
        .threshold(threshold)
        .time_range(after_ts, before_ts)
        .execute()
        .await?;

    // 显示结果
    if results.is_empty() {
        output.info(&format!(
            "No results found above threshold {:.2}",
            threshold
        ));
    } else {
        output.search_results(&results);
    }

    Ok(())
}

/// 解析日期时间字符串
/// 支持格式：
/// - YYYY-MM-DD
/// - YYYY-MM-DD HH:MM
fn parse_datetime(input: &str) -> Result<i64> {
    // 尝试解析 YYYY-MM-DD HH:MM 格式
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M") {
        return Ok(dt.and_utc().timestamp_millis());
    }

    // 尝试解析 YYYY-MM-DD 格式（默认为当天 00:00）
    if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let dt = date
            .and_hms_opt(0, 0, 0)
            .context("Failed to create datetime")?;
        return Ok(dt.and_utc().timestamp_millis());
    }

    anyhow::bail!("Invalid date format. Use YYYY-MM-DD or YYYY-MM-DD HH:MM")
}

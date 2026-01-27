use anyhow::{Context, Result};
use chrono::NaiveDateTime;

use crate::config::Config;
use crate::embedding::EmbeddingModel;
use crate::ui::Output;
use memo_local::LocalStorageClient;
use memo_types::{StorageBackend, StorageConfig, TimeRange};

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

    // 检查 API key（Ollama 不需要）
    config.validate_api_key(force_local)?;

    // 创建 embedding 模型
    let model = EmbeddingModel::new(
        config.embedding_api_key.clone(),
        config.embedding_model.clone(),
        config.embedding_base_url.clone(),
        config.embedding_dimension,
        config.embedding_provider.clone(),
    )?;

    // 创建存储客户端
    let storage_config = StorageConfig {
        path: config.brain_path.to_string_lossy().to_string(),
        dimension: model.dimension(),
    };
    let storage = LocalStorageClient::connect(&storage_config).await?;
    let record_count = storage.count().await?;

    // 显示数据库信息
    output.database_info(&config.brain_path, record_count);

    output.status("Encoding", "query");

    // 生成查询向量
    let query_vector = model.encode(query).await?;

    output.status("Searching", "database");

    // 解析时间过滤参数
    let time_range = if after.is_some() || before.is_some() {
        let after_ts = after.as_ref().map(|s| parse_datetime(s)).transpose()?;
        let before_ts = before.as_ref().map(|s| parse_datetime(s)).transpose()?;
        Some(TimeRange {
            after: after_ts,
            before: before_ts,
        })
    } else {
        None
    };

    // 使用向量搜索
    let results = storage
        .search_by_vector(query_vector, limit, threshold, time_range)
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

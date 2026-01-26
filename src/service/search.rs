use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime};
use console::Style;
use futures::TryStreamExt;
use lancedb::query::{ExecutableQuery, QueryBase};

use crate::config::Config;
use crate::db::{Connection, TableOperations};
use crate::embedding::EmbeddingModel;
use crate::output::Output;

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
    let conn = Connection::connect(config.brain_path.to_str().unwrap()).await?;
    let table = TableOperations::open_table(conn.inner(), "memories").await?;
    let record_count = table.count_rows(None).await.unwrap_or(0);

    // 检查 API key（Ollama 不需要）
    let is_ollama = config
        .embedding_provider
        .as_ref()
        .map(|p| p.to_lowercase() == "ollama")
        .unwrap_or_else(|| {
            config
                .embedding_base_url
                .as_ref()
                .map(|url| url.contains("ollama") || url.contains("11434"))
                .unwrap_or(false)
        });

    if !is_ollama && config.embedding_api_key.is_empty() {
        eprintln!();
        output.warning("Embedding API key not configured");
        eprintln!();
        eprintln!("  请运行以下命令创建配置文件：");
        eprintln!("    {}", Style::new().cyan().apply_to("memo init"));
        eprintln!();
        eprintln!("  然后编辑配置文件并设置你的 API key：");
        eprintln!(
            "    {}",
            Style::new().dim().apply_to(if force_local {
                "./.memo/config.toml"
            } else {
                "~/.memo/config.toml"
            })
        );
        eprintln!();
        eprintln!("  配置示例：");
        eprintln!(
            "    {}",
            Style::new()
                .dim()
                .apply_to("embedding_api_key = \"sk-...\"")
        );
        eprintln!(
            "    {}",
            Style::new()
                .dim()
                .apply_to("embedding_model = \"text-embedding-3-small\"")
        );
        eprintln!();
        anyhow::bail!("Missing required configuration");
    }

    // 显示数据库信息
    output.database_info(&config.brain_path, record_count);
    eprintln!();

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
    eprintln!();

    // 如果有时间过滤，增加查询限制以便后续过滤
    let query_limit = if after.is_some() || before.is_some() {
        limit * 10 // 获取更多结果用于过滤
    } else {
        limit
    };

    // 只查询需要的列，提升性能
    let mut stream = table
        .vector_search(query_vector)?
        .select(lancedb::query::Select::columns(&[
            "title",
            "content",
            "updated_at",
            "_distance", // 必须包含 _distance 用于相似度计算
        ]))
        .limit(query_limit)
        .execute()
        .await?;

    let batch = stream.try_next().await?.context("未找到任何结果")?;

    let titles = batch
        .column_by_name("title")
        .and_then(|c| c.as_any().downcast_ref::<arrow_array::StringArray>())
        .context("Failed to get title column")?;

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

    let distances = batch
        .column_by_name("_distance")
        .and_then(|c| c.as_any().downcast_ref::<arrow_array::Float32Array>())
        .context("Failed to get distance column")?;

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

    // 收集所有结果到 Vec 以便过滤和排序
    let mut results: Vec<(f32, String, i64, String)> = Vec::new();

    for i in 0..batch.num_rows() {
        let distance = distances.value(i);
        // 将 L2 距离转换为相似度分数 (0-1)
        let score = (1.0 - (distance / 2.0)).max(0.0);

        if score < threshold {
            continue;
        }

        let timestamp = updated_ats.value(i);

        // 时间范围过滤
        if let Some(after_ts) = after_ts {
            if timestamp < after_ts {
                continue;
            }
        }

        if let Some(before_ts) = before_ts {
            if timestamp > before_ts {
                continue;
            }
        }

        results.push((
            score,
            titles.value(i).to_string(),
            timestamp,
            contents.value(i).to_string(),
        ));
    }

    // 限制结果数量
    results.truncate(limit);

    // 显示结果
    for (score, title, timestamp, content) in &results {
        let updated = DateTime::from_timestamp_millis(*timestamp)
            .map(|dt| dt.format("%Y-%m-%d %H:%M").to_string())
            .unwrap_or_else(|| "N/A".to_string());

        output.search_result(*score, title, &updated, content);
    }

    if results.is_empty() {
        output.info(&format!(
            "No results found above threshold {:.2}",
            threshold
        ));
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
        let dt = date.and_hms_opt(0, 0, 0).unwrap();
        return Ok(dt.and_utc().timestamp_millis());
    }

    anyhow::bail!("Invalid date format. Use YYYY-MM-DD or YYYY-MM-DD HH:MM")
}

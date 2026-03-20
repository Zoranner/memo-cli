//! CLI 日期参数解析为存储层时间范围

use anyhow::{Context, Result};
use chrono::NaiveDateTime;
use memo_types::TimeRange;

pub fn parse_cli_time_range(
    after: Option<String>,
    before: Option<String>,
) -> Result<Option<TimeRange>> {
    if after.is_none() && before.is_none() {
        return Ok(None);
    }

    let after_ts = after.as_ref().map(|s| parse_datetime(s)).transpose()?;
    let before_ts = before.as_ref().map(|s| parse_datetime(s)).transpose()?;

    Ok(Some(TimeRange {
        after: after_ts,
        before: before_ts,
    }))
}

fn parse_datetime(input: &str) -> Result<i64> {
    if let Ok(dt) = NaiveDateTime::parse_from_str(input, "%Y-%m-%d %H:%M") {
        return Ok(dt.and_utc().timestamp_millis());
    }

    if let Ok(date) = chrono::NaiveDate::parse_from_str(input, "%Y-%m-%d") {
        let dt = date
            .and_hms_opt(0, 0, 0)
            .context("Failed to create datetime")?;
        return Ok(dt.and_utc().timestamp_millis());
    }

    anyhow::bail!("Invalid date format. Use YYYY-MM-DD or YYYY-MM-DD HH:MM")
}

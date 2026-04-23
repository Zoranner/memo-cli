use anyhow::{Context, Result};
use chrono::Utc;
use serde_json::Value;

use crate::types::MemoryRecord;

pub(crate) fn normalize_text(text: &str) -> String {
    text.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

pub(super) fn now_ts() -> i64 {
    Utc::now().timestamp_millis()
}

pub(super) fn vec_to_json(vector: &[f32]) -> Result<String> {
    Ok(serde_json::to_string(vector)?)
}

pub(super) fn json_to_vec(raw: &str) -> Result<Vec<f32>> {
    let value: Value = serde_json::from_str(raw)?;
    let array = value
        .as_array()
        .context("vector json is not an array")?
        .iter()
        .map(|item| item.as_f64().unwrap_or_default() as f32)
        .collect();
    Ok(array)
}

pub(super) fn memory_key(kind: &str, id: &str) -> String {
    format!("{}:{}", kind, id)
}

pub(super) fn sort_l3_records(records: &mut [MemoryRecord]) {
    records.sort_by(|left, right| {
        right
            .hit_count()
            .cmp(&left.hit_count())
            .then_with(|| right.activity_at().cmp(&left.activity_at()))
            .then_with(|| left.id().cmp(right.id()))
    });
}

pub(super) fn count_table(conn: &rusqlite::Connection, table: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {}", table);
    conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map(|count| count.max(0) as usize)
        .map_err(Into::into)
}

pub(super) fn table_for_kind(kind: &str) -> Result<&'static str> {
    match kind {
        "episode" => Ok("episodes"),
        "entity" => Ok("entities"),
        "fact" => Ok("facts"),
        "edge" => Ok("edges"),
        _ => anyhow::bail!("unsupported memory kind: {}", kind),
    }
}

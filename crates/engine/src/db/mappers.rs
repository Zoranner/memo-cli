use anyhow::Result;
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection};

use crate::types::{EdgeRecord, EpisodeRecord, FactRecord, MemoryLayer};

pub(super) fn map_episode(row: &rusqlite::Row<'_>) -> rusqlite::Result<EpisodeRecord> {
    Ok(EpisodeRecord {
        id: row.get(0)?,
        content: row.get(1)?,
        layer: parse_layer(row.get::<_, String>(2)?, 2)?,
        confidence: row.get(3)?,
        source_episode_id: row.get(4)?,
        session_id: row.get(5)?,
        created_at: ts_to_dt(row.get(6)?),
        updated_at: ts_to_dt(row.get(7)?),
        last_seen_at: ts_to_dt(row.get(8)?),
        archived_at: row.get::<_, Option<i64>>(9)?.map(ts_to_dt),
        invalidated_at: row.get::<_, Option<i64>>(10)?.map(ts_to_dt),
        hit_count: row.get::<_, i64>(11)?.max(0) as u64,
    })
}

pub(super) fn map_fact(row: &rusqlite::Row<'_>) -> rusqlite::Result<FactRecord> {
    Ok(FactRecord {
        id: row.get(0)?,
        subject_entity_id: row.get(1)?,
        subject_text: row.get(2)?,
        predicate: row.get(3)?,
        object_entity_id: row.get(4)?,
        object_text: row.get(5)?,
        layer: parse_layer(row.get::<_, String>(6)?, 6)?,
        confidence: row.get(7)?,
        source_episode_id: row.get(8)?,
        valid_from: row.get::<_, Option<i64>>(9)?.map(ts_to_dt),
        valid_to: row.get::<_, Option<i64>>(10)?.map(ts_to_dt),
        created_at: ts_to_dt(row.get(11)?),
        updated_at: ts_to_dt(row.get(12)?),
        archived_at: row.get::<_, Option<i64>>(13)?.map(ts_to_dt),
        invalidated_at: row.get::<_, Option<i64>>(14)?.map(ts_to_dt),
        hit_count: row.get::<_, i64>(15)?.max(0) as u64,
    })
}

pub(super) fn map_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<EdgeRecord> {
    Ok(EdgeRecord {
        id: row.get(0)?,
        subject_entity_id: row.get(1)?,
        predicate: row.get(2)?,
        object_entity_id: row.get(3)?,
        weight: row.get(4)?,
        source_episode_id: row.get(5)?,
        layer: parse_layer(row.get::<_, String>(6)?, 6)?,
        valid_from: row.get::<_, Option<i64>>(7)?.map(ts_to_dt),
        valid_to: row.get::<_, Option<i64>>(8)?.map(ts_to_dt),
        created_at: ts_to_dt(row.get(9)?),
        updated_at: ts_to_dt(row.get(10)?),
        archived_at: row.get::<_, Option<i64>>(11)?.map(ts_to_dt),
        invalidated_at: row.get::<_, Option<i64>>(12)?.map(ts_to_dt),
        hit_count: row.get::<_, i64>(13)?.max(0) as u64,
    })
}

pub(super) fn load_aliases(conn: &Connection, entity_id: &str) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT alias FROM entity_aliases WHERE entity_id = ?1 ORDER BY alias ASC")?;
    let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
    let aliases = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(aliases)
}

pub(super) fn ts_to_dt(ts: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ts).unwrap_or_else(Utc::now)
}

pub(super) fn to_sql_error(error: anyhow::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

fn parse_layer(raw: String, column: usize) -> rusqlite::Result<MemoryLayer> {
    raw.parse::<MemoryLayer>().map_err(|error: anyhow::Error| {
        rusqlite::Error::FromSqlConversionFailure(
            column,
            rusqlite::types::Type::Text,
            Box::new(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                error.to_string(),
            )),
        )
    })
}

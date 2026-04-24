use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

mod connection;
mod index_jobs;
mod index_state;
mod layers;
mod mappers;
mod read;
mod schema;
mod search;
mod support;
mod write;

#[cfg(test)]
mod tests;

use crate::types::{
    EdgeRecord, EntityInput, EntityRecord, EpisodeInput, EpisodeRecord, FactInput, FactRecord,
    IndexStatus, LayerSummary, MemoryLayer, MemoryRecord,
};
use index_jobs::{
    clear_index_jobs_by_ids, fail_index_jobs_by_ids, index_job_observability,
    queue_index_delete_jobs, queue_text_index_job, queue_vector_index_job, record_index_ready,
};
use mappers::{load_aliases, map_edge, map_episode, map_fact, to_sql_error, ts_to_dt};
use schema::init_schema;
use support::{
    count_table, json_to_vec, memory_key, now_ts, sort_l3_records, table_for_kind, vec_to_json,
};

pub(crate) use support::normalize_text;

pub struct Database {
    conn: Mutex<Connection>,
}

pub struct ObservationContext<'a> {
    pub source_episode_id: Option<&'a str>,
    pub observed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IndexJobOperation {
    Upsert,
    Delete,
}

impl IndexJobOperation {
    fn as_str(self) -> &'static str {
        match self {
            Self::Upsert => "upsert",
            Self::Delete => "delete",
        }
    }
}

impl std::str::FromStr for IndexJobOperation {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "upsert" => Ok(Self::Upsert),
            "delete" => Ok(Self::Delete),
            _ => anyhow::bail!("invalid index job operation: {}", s),
        }
    }
}

#[derive(Debug, Clone)]
pub struct IndexJobRecord {
    pub id: String,
    pub memory_kind: String,
    pub memory_id: String,
    pub operation: IndexJobOperation,
    pub failed: bool,
}

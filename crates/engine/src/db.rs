use std::{
    collections::{HashMap, HashSet},
    sync::Mutex,
};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

use crate::types::{
    EdgeRecord, EntityInput, EntityRecord, EpisodeInput, EpisodeRecord, FactInput, FactRecord,
    IndexStatus, LayerSummary, MemoryLayer, MemoryRecord,
};

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum IndexJobStatus {
    Pending,
    Failed,
}

impl IndexJobStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Failed => "failed",
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

impl Database {
    pub fn open(path: &std::path::Path) -> Result<Self> {
        let conn = Connection::open(path)?;
        conn.pragma_update(None, "foreign_keys", "ON")?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        init_schema(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    pub fn insert_episode(
        &self,
        input: &EpisodeInput,
        vector: Option<&[f32]>,
    ) -> Result<EpisodeRecord> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let now = input
            .recorded_at
            .map(|ts| ts.timestamp_millis())
            .unwrap_or_else(now_ts);
        let id = Uuid::new_v4().to_string();
        let vector_json = vector.map(vec_to_json).transpose()?;
        conn.execute(
            "INSERT INTO episodes
             (id, content, normalized_content, layer, confidence, source_episode_id, session_id, created_at, updated_at, last_seen_at, hit_count, vector_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?8, 0, ?9)",
            params![
                id,
                input.content,
                normalize_text(&input.content),
                input.layer.as_str(),
                input.confidence,
                input.source_episode_id,
                input.session_id,
                now,
                vector_json
            ],
        )?;

        conn.execute(
            "INSERT INTO memory_layers (memory_id, memory_kind, layer, status, created_at, updated_at)
             VALUES (?1, 'episode', ?2, 'active', ?3, ?3)",
            params![id, input.layer.as_str(), now],
        )?;
        queue_text_index_job(&conn, "episode", &id, IndexJobOperation::Upsert)?;
        if vector_json.is_some() {
            queue_vector_index_job(&conn, "episode", &id, IndexJobOperation::Upsert)?;
        }

        drop(conn);
        self.get_episode(&id)?
            .context("failed to load inserted episode")
    }

    pub fn upsert_entity(
        &self,
        input: &EntityInput,
        layer: MemoryLayer,
        observation: ObservationContext<'_>,
        vector: Option<&[f32]>,
    ) -> Result<EntityRecord> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let observed_at_ts = observation.observed_at.timestamp_millis();
        let normalized_name = normalize_text(&input.name);
        let has_vector = vector.is_some();
        let existing_id = find_active_entity_id_by_reference(&conn, &normalized_name)?;

        let entity_id = if let Some(existing_id) = existing_id.clone() {
            conn.execute(
                "UPDATE entities
                 SET confidence = MAX(confidence, ?2),
                     entity_type = CASE
                         WHEN entity_type = 'unknown' AND ?4 <> 'unknown' THEN ?4
                         ELSE entity_type
                     END,
                     updated_at = MAX(updated_at, ?3),
                     last_seen_at = MAX(last_seen_at, ?3)
                 WHERE id = ?1",
                params![
                    existing_id,
                    input.confidence,
                    observed_at_ts,
                    input.entity_type
                ],
            )?;
            existing_id
        } else {
            let entity_id = Uuid::new_v4().to_string();
            let vector_json = vector.map(vec_to_json).transpose()?;
            conn.execute(
                "INSERT INTO entities
                 (id, entity_type, canonical_name, normalized_name, confidence, source_episode_id, created_at, updated_at, last_seen_at, hit_count, vector_json)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7, ?7, 0, ?8)",
                params![
                    entity_id,
                    input.entity_type,
                    input.name,
                    normalized_name,
                    input.confidence,
                    observation.source_episode_id,
                    observed_at_ts,
                    vector_json
                ],
            )?;
            conn.execute(
                "INSERT INTO memory_layers (memory_id, memory_kind, layer, status, created_at, updated_at)
                 VALUES (?1, 'entity', ?2, 'active', ?3, ?3)",
                params![entity_id, layer.as_str(), observed_at_ts],
            )?;
            entity_id
        };

        for alias in &input.aliases {
            conn.execute(
                "INSERT OR IGNORE INTO entity_aliases
                 (id, entity_id, alias, normalized_alias, created_at)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    Uuid::new_v4().to_string(),
                    entity_id,
                    alias,
                    normalize_text(alias),
                    observed_at_ts
                ],
            )?;
        }

        queue_text_index_job(&conn, "entity", &entity_id, IndexJobOperation::Upsert)?;
        if existing_id.is_none() && has_vector {
            queue_vector_index_job(&conn, "entity", &entity_id, IndexJobOperation::Upsert)?;
        }

        drop(conn);
        self.get_entity(&entity_id)?
            .context("failed to load upserted entity")
    }

    pub fn add_mention(
        &self,
        episode_id: &str,
        entity_id: &str,
        role: &str,
        confidence: f32,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.execute(
            "INSERT INTO mentions
             (id, episode_id, entity_id, role, confidence, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                episode_id,
                entity_id,
                role,
                confidence,
                now_ts()
            ],
        )?;
        Ok(())
    }

    pub fn ensure_mention(
        &self,
        episode_id: &str,
        entity_id: &str,
        role: &str,
        confidence: f32,
    ) -> Result<bool> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let inserted = conn.execute(
            "INSERT INTO mentions
             (id, episode_id, entity_id, role, confidence, created_at)
             SELECT ?1, ?2, ?3, ?4, ?5, ?6
             WHERE NOT EXISTS (
                 SELECT 1
                 FROM mentions
                 WHERE episode_id = ?2
                   AND entity_id = ?3
                   AND role = ?4
             )",
            params![
                Uuid::new_v4().to_string(),
                episode_id,
                entity_id,
                role,
                confidence,
                now_ts()
            ],
        )?;
        Ok(inserted > 0)
    }

    pub fn insert_fact(
        &self,
        input: &FactInput,
        layer: MemoryLayer,
        subject_entity_id: Option<&str>,
        object_entity_id: Option<&str>,
        observation: ObservationContext<'_>,
        vector: Option<&[f32]>,
    ) -> Result<FactRecord> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let observed_at_ts = observation.observed_at.timestamp_millis();
        let vector_json = vector.map(vec_to_json).transpose()?;
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO facts
             (id, subject_entity_id, subject_text, predicate, object_entity_id, object_text, confidence, source_episode_id, layer, valid_from, created_at, updated_at, hit_count, vector_json)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10, ?10, 0, ?11)",
            params![
                id,
                subject_entity_id,
                input.subject,
                input.predicate,
                object_entity_id,
                input.object,
                input.confidence,
                observation.source_episode_id,
                layer.as_str(),
                observed_at_ts,
                vector_json
            ],
        )?;
        conn.execute(
            "INSERT INTO memory_layers (memory_id, memory_kind, layer, status, created_at, updated_at)
             VALUES (?1, 'fact', ?2, 'active', ?3, ?3)",
            params![id, layer.as_str(), observed_at_ts],
        )?;
        queue_text_index_job(&conn, "fact", &id, IndexJobOperation::Upsert)?;
        if vector_json.is_some() {
            queue_vector_index_job(&conn, "fact", &id, IndexJobOperation::Upsert)?;
        }
        drop(conn);
        self.get_fact(&id)?.context("failed to load inserted fact")
    }

    pub fn insert_edge(
        &self,
        subject_entity_id: &str,
        predicate: &str,
        object_entity_id: &str,
        weight: f32,
        layer: MemoryLayer,
        observation: ObservationContext<'_>,
    ) -> Result<EdgeRecord> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let observed_at_ts = observation.observed_at.timestamp_millis();
        let id = Uuid::new_v4().to_string();
        conn.execute(
            "INSERT INTO edges
             (id, subject_entity_id, predicate, object_entity_id, weight, source_episode_id, layer, valid_from, created_at, updated_at, hit_count)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?8, ?8, 0)",
            params![
                id,
                subject_entity_id,
                predicate,
                object_entity_id,
                weight,
                observation.source_episode_id,
                layer.as_str(),
                observed_at_ts
            ],
        )?;
        conn.execute(
            "INSERT INTO memory_layers (memory_id, memory_kind, layer, status, created_at, updated_at)
             VALUES (?1, 'edge', ?2, 'active', ?3, ?3)",
            params![id, layer.as_str(), observed_at_ts],
        )?;
        drop(conn);
        self.get_edge(&id)?.context("failed to load inserted edge")
    }

    pub fn get_memory(&self, id: &str) -> Result<Option<MemoryRecord>> {
        if let Some(record) = self.get_episode(id)? {
            return Ok(Some(MemoryRecord::Episode(record)));
        }
        if let Some(record) = self.get_entity(id)? {
            return Ok(Some(MemoryRecord::Entity(record)));
        }
        if let Some(record) = self.get_fact(id)? {
            return Ok(Some(MemoryRecord::Fact(record)));
        }
        if let Some(record) = self.get_edge(id)? {
            return Ok(Some(MemoryRecord::Edge(record)));
        }
        Ok(None)
    }

    pub fn get_memory_by_kind(&self, kind: &str, id: &str) -> Result<Option<MemoryRecord>> {
        match kind {
            "episode" => Ok(self.get_episode(id)?.map(MemoryRecord::Episode)),
            "entity" => Ok(self.get_entity(id)?.map(MemoryRecord::Entity)),
            "fact" => Ok(self.get_fact(id)?.map(MemoryRecord::Fact)),
            "edge" => Ok(self.get_edge(id)?.map(MemoryRecord::Edge)),
            other => anyhow::bail!("unsupported memory kind: {}", other),
        }
    }

    pub fn get_active_memory(&self, id: &str) -> Result<Option<MemoryRecord>> {
        Ok(self.get_memory(id)?.filter(MemoryRecord::is_active))
    }

    pub fn get_active_memory_by_kind(&self, kind: &str, id: &str) -> Result<Option<MemoryRecord>> {
        Ok(self
            .get_memory_by_kind(kind, id)?
            .filter(MemoryRecord::is_active))
    }

    pub fn get_episode(&self, id: &str) -> Result<Option<EpisodeRecord>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.query_row(
            "SELECT id, content, layer, confidence, source_episode_id, session_id, created_at, updated_at, last_seen_at,
                    archived_at, invalidated_at, hit_count
             FROM episodes WHERE id = ?1 LIMIT 1",
            params![id],
            map_episode,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn load_unstructured_episodes(&self, layers: &[MemoryLayer]) -> Result<Vec<EpisodeRecord>> {
        if layers.is_empty() {
            return Ok(Vec::new());
        }

        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let placeholders = (1..=layers.len())
            .map(|index| format!("?{}", index))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT ep.id, ep.content, ep.layer, ep.confidence, ep.source_episode_id, ep.session_id,
                    ep.created_at, ep.updated_at, ep.last_seen_at, ep.archived_at, ep.invalidated_at, ep.hit_count
             FROM episodes ep
             WHERE ep.archived_at IS NULL
               AND ep.invalidated_at IS NULL
               AND ep.structured_at IS NULL
               AND ep.layer IN ({})
               AND NOT EXISTS (
                    SELECT 1
                    FROM entities e
                    WHERE e.source_episode_id = ep.id
                      AND e.archived_at IS NULL
                      AND e.invalidated_at IS NULL
               )
               AND NOT EXISTS (
                    SELECT 1
                    FROM facts f
                    WHERE f.source_episode_id = ep.id
                      AND f.archived_at IS NULL
                      AND f.invalidated_at IS NULL
               )
               AND NOT EXISTS (
                    SELECT 1
                    FROM edges ed
                    WHERE ed.source_episode_id = ep.id
                      AND ed.archived_at IS NULL
                      AND ed.invalidated_at IS NULL
               )
               AND NOT EXISTS (
                    SELECT 1
                    FROM mentions m
                    WHERE m.episode_id = ep.id
               )
             ORDER BY ep.created_at ASC",
            placeholders
        );
        let args = layers
            .iter()
            .map(|layer| layer.as_str())
            .collect::<Vec<_>>();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), map_episode)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn mark_episode_structured(&self, episode_id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.execute(
            "UPDATE episodes
             SET structured_at = COALESCE(structured_at, ?2)
             WHERE id = ?1",
            params![episode_id, now_ts()],
        )?;
        Ok(())
    }

    pub fn get_entity(&self, id: &str) -> Result<Option<EntityRecord>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let row = conn
            .query_row(
                "SELECT id, entity_type, canonical_name, confidence, source_episode_id, layer,
                        created_at, updated_at, last_seen_at, archived_at, invalidated_at, hit_count
                 FROM entities WHERE id = ?1 LIMIT 1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, f32>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        row.get::<_, Option<i64>>(10)?,
                        row.get::<_, i64>(11)?,
                    ))
                },
            )
            .optional()?;
        let Some(row) = row else {
            return Ok(None);
        };
        let aliases = load_aliases(&conn, &row.0)?;
        Ok(Some(EntityRecord {
            id: row.0,
            entity_type: row.1,
            canonical_name: row.2,
            aliases,
            layer: row.5.parse()?,
            confidence: row.3,
            source_episode_id: row.4,
            created_at: ts_to_dt(row.6),
            updated_at: ts_to_dt(row.7),
            last_seen_at: ts_to_dt(row.8),
            archived_at: row.9.map(ts_to_dt),
            invalidated_at: row.10.map(ts_to_dt),
            hit_count: row.11.max(0) as u64,
        }))
    }

    pub fn get_fact(&self, id: &str) -> Result<Option<FactRecord>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.query_row(
            "SELECT id, subject_entity_id, subject_text, predicate, object_entity_id, object_text,
                    layer, confidence, source_episode_id, valid_from, valid_to, created_at, updated_at, archived_at, invalidated_at, hit_count
             FROM facts WHERE id = ?1 LIMIT 1",
            params![id],
            map_fact,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn get_edge(&self, id: &str) -> Result<Option<EdgeRecord>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.query_row(
            "SELECT id, subject_entity_id, predicate, object_entity_id, weight, source_episode_id,
                    layer, valid_from, valid_to, created_at, updated_at, archived_at, invalidated_at, hit_count
             FROM edges WHERE id = ?1 LIMIT 1",
            params![id],
            map_edge,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn search_exact_alias(&self, query: &str) -> Result<Vec<MemoryRecord>> {
        let normalized = normalize_text(query);
        let (entity_ids, episode_ids) = {
            let conn = self.conn.lock().expect("sqlite mutex poisoned");

            let mut entity_stmt = conn.prepare(
                "SELECT DISTINCT e.id
                 FROM entities e
                 LEFT JOIN entity_aliases a ON a.entity_id = e.id
                 WHERE e.archived_at IS NULL
                   AND e.invalidated_at IS NULL
                   AND (e.normalized_name = ?1 OR a.normalized_alias = ?1)",
            )?;
            let entity_ids: Vec<String> = entity_stmt
                .query_map(params![normalized], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(entity_stmt);

            let mut episode_stmt = conn.prepare(
                "SELECT id FROM episodes
                 WHERE archived_at IS NULL
                   AND invalidated_at IS NULL
                   AND normalized_content = ?1",
            )?;
            let episode_ids: Vec<String> = episode_stmt
                .query_map(params![normalize_text(query)], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (entity_ids, episode_ids)
        };

        let mut result = Vec::new();
        for entity_id in entity_ids {
            if let Some(record) = self.get_active_memory(&entity_id)? {
                result.push(record);
            }
        }
        for episode_id in episode_ids {
            if let Some(record) = self.get_active_memory(&episode_id)? {
                result.push(record);
            }
        }

        Ok(result)
    }

    pub fn resolve_active_entity_reference(
        &self,
        name_or_alias: &str,
    ) -> Result<Option<EntityRecord>> {
        let normalized = normalize_text(name_or_alias);
        let entity_id = {
            let conn = self.conn.lock().expect("sqlite mutex poisoned");
            find_active_entity_id_by_reference(&conn, &normalized)?
        };

        match entity_id {
            Some(id) => self.get_entity(&id),
            None => Ok(None),
        }
    }

    pub fn related_graph_records(
        &self,
        entity_ids: &[String],
        hops: usize,
        limit: usize,
    ) -> Result<Vec<(MemoryRecord, usize)>> {
        if entity_ids.is_empty() || hops == 0 || limit == 0 {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut all_records = Vec::new();
        let mut frontier: HashSet<String> = entity_ids.iter().cloned().collect();
        let mut visited: HashSet<String> = entity_ids.iter().cloned().collect();
        let mut seen_records = HashSet::new();

        for hop in 1..=hops {
            if frontier.is_empty() {
                break;
            }
            let current: Vec<String> = frontier.drain().collect();
            let mut next_frontier = HashSet::new();
            let mut added_this_hop = 0usize;
            let placeholders = (1..=current.len())
                .map(|index| format!("?{}", index))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT id, subject_entity_id, predicate, object_entity_id, weight, source_episode_id,
                        layer, valid_from, valid_to, created_at, updated_at, archived_at, invalidated_at, hit_count
                 FROM edges
                 WHERE archived_at IS NULL
                   AND invalidated_at IS NULL
                   AND (subject_entity_id IN ({0}) OR object_entity_id IN ({0}))
                 ORDER BY rowid ASC",
                placeholders
            );
            let mut edge_stmt = conn.prepare(&sql)?;
            let edges = edge_stmt
                .query_map(rusqlite::params_from_iter(current.iter()), map_edge)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for edge in edges {
                if added_this_hop >= limit {
                    break;
                }
                if !seen_records.insert(memory_key("edge", &edge.id)) {
                    continue;
                }
                let subject = edge.subject_entity_id.clone();
                let object = edge.object_entity_id.clone();
                if visited.insert(subject.clone()) {
                    next_frontier.insert(subject);
                }
                if visited.insert(object.clone()) {
                    next_frontier.insert(object);
                }
                all_records.push((MemoryRecord::Edge(edge), hop));
                added_this_hop += 1;
            }

            let sql = format!(
                "SELECT id, subject_entity_id, subject_text, predicate, object_entity_id, object_text,
                        layer, confidence, source_episode_id, valid_from, valid_to, created_at, updated_at, archived_at, invalidated_at, hit_count
                 FROM facts
                 WHERE archived_at IS NULL
                   AND invalidated_at IS NULL
                   AND ((subject_entity_id IS NOT NULL AND subject_entity_id IN ({0}))
                     OR (object_entity_id IS NOT NULL AND object_entity_id IN ({0})))
                 ORDER BY rowid ASC",
                placeholders
            );
            let mut fact_stmt = conn.prepare(&sql)?;
            let facts = fact_stmt
                .query_map(rusqlite::params_from_iter(current.iter()), map_fact)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for fact in facts {
                if added_this_hop >= limit {
                    break;
                }
                if !seen_records.insert(memory_key("fact", &fact.id)) {
                    continue;
                }
                all_records.push((MemoryRecord::Fact(fact), hop));
                added_this_hop += 1;
            }

            frontier = next_frontier;
        }

        Ok(all_records)
    }

    pub fn load_search_documents(&self) -> Result<Vec<(String, String, String, String)>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut docs = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT id, content, layer FROM episodes WHERE archived_at IS NULL AND invalidated_at IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                "episode".to_string(),
                row.get::<_, String>(2)?,
                row.get::<_, String>(1)?,
            ))
        })?;
        for row in rows {
            docs.push(row?);
        }

        let mut stmt = conn.prepare(
            "SELECT id, canonical_name, layer FROM entities WHERE archived_at IS NULL AND invalidated_at IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                "entity".to_string(),
                row.get::<_, String>(2)?,
                row.get::<_, String>(1)?,
            ))
        })?;
        for row in rows {
            let (id, kind, layer, mut body) = row?;
            let aliases = load_aliases(&conn, &id)?;
            if !aliases.is_empty() {
                body.push(' ');
                body.push_str(&aliases.join(" "));
            }
            docs.push((id, kind, layer, body));
        }

        let mut stmt = conn.prepare(
            "SELECT id, subject_text, predicate, object_text, layer FROM facts WHERE archived_at IS NULL AND invalidated_at IS NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            let text = format!(
                "{} {} {}",
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?
            );
            Ok((
                row.get::<_, String>(0)?,
                "fact".to_string(),
                row.get::<_, String>(4)?,
                text,
            ))
        })?;
        for row in rows {
            docs.push(row?);
        }

        Ok(docs)
    }

    pub fn load_vector_documents(&self) -> Result<Vec<(String, String, Vec<f32>)>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut docs = Vec::new();

        let mut stmt = conn.prepare(
            "SELECT id, vector_json FROM episodes WHERE archived_at IS NULL AND invalidated_at IS NULL AND vector_json IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                "episode".to_string(),
                row.get::<_, String>(1)?,
            ))
        })?;
        for row in rows {
            let (id, kind, raw) = row?;
            docs.push((id, kind, json_to_vec(&raw)?));
        }

        let mut stmt = conn.prepare(
            "SELECT id, vector_json FROM entities WHERE archived_at IS NULL AND invalidated_at IS NULL AND vector_json IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                "entity".to_string(),
                row.get::<_, String>(1)?,
            ))
        })?;
        for row in rows {
            let (id, kind, raw) = row?;
            docs.push((id, kind, json_to_vec(&raw)?));
        }

        let mut stmt = conn.prepare(
            "SELECT id, vector_json FROM facts WHERE archived_at IS NULL AND invalidated_at IS NULL AND vector_json IS NOT NULL",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                "fact".to_string(),
                row.get::<_, String>(1)?,
            ))
        })?;
        for row in rows {
            let (id, kind, raw) = row?;
            docs.push((id, kind, json_to_vec(&raw)?));
        }

        Ok(docs)
    }

    pub fn load_search_document(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Option<(String, String, String, String)>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        load_search_document_record(&conn, kind, id)
    }

    pub fn load_vector_document(
        &self,
        kind: &str,
        id: &str,
    ) -> Result<Option<(String, String, Vec<f32>)>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        load_vector_document_record(&conn, kind, id)
    }

    pub fn load_outstanding_index_jobs(&self, index_name: &str) -> Result<Vec<IndexJobRecord>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id, memory_kind, memory_id, operation, status
             FROM index_jobs
             WHERE index_name = ?1
               AND status IN ('pending', 'failed')
             ORDER BY CASE status WHEN 'pending' THEN 0 ELSE 1 END, updated_at ASC, created_at ASC",
        )?;
        let rows = stmt.query_map(params![index_name], |row| {
            Ok(IndexJobRecord {
                id: row.get(0)?,
                memory_kind: row.get(1)?,
                memory_id: row.get(2)?,
                operation: row.get::<_, String>(3)?.parse().map_err(to_sql_error)?,
                failed: row.get::<_, String>(4)? == IndexJobStatus::Failed.as_str(),
            })
        })?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn clear_index_jobs(&self, index_name: &str, job_ids: &[String]) -> Result<()> {
        if job_ids.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        clear_index_jobs_by_ids(&conn, index_name, job_ids)
    }

    pub fn clear_all_index_jobs(&self, index_name: &str) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.execute(
            "DELETE FROM index_jobs WHERE index_name = ?1",
            params![index_name],
        )?;
        Ok(())
    }

    pub fn fail_index_jobs(
        &self,
        index_name: &str,
        job_ids: &[String],
        detail: &str,
    ) -> Result<()> {
        if job_ids.is_empty() {
            return Ok(());
        }

        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        fail_index_jobs_by_ids(&conn, index_name, job_ids, detail)
    }

    pub fn load_l3_records(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        if limit == 0 {
            return Ok(Vec::new());
        }
        let rows = {
            let conn = self.conn.lock().expect("sqlite mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT memory_id, memory_kind FROM memory_layers
                 WHERE layer = 'L3' AND status = 'active'
                 ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut records = Vec::new();
        for (id, kind) in rows {
            let record = match kind.as_str() {
                "episode" => self.get_episode(&id)?.map(MemoryRecord::Episode),
                "entity" => self.get_entity(&id)?.map(MemoryRecord::Entity),
                "fact" => self.get_fact(&id)?.map(MemoryRecord::Fact),
                "edge" => self.get_edge(&id)?.map(MemoryRecord::Edge),
                _ => None,
            };
            if let Some(record) = record {
                records.push(record);
            }
        }

        sort_l3_records(&mut records);
        if records.len() > limit {
            records.truncate(limit);
        }

        Ok(records)
    }

    pub fn load_all_l3_records(&self) -> Result<Vec<MemoryRecord>> {
        let rows = {
            let conn = self.conn.lock().expect("sqlite mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT memory_id, memory_kind FROM memory_layers
                 WHERE layer = 'L3' AND status = 'active'
                 ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()?
        };

        let mut records = Vec::new();
        for (id, kind) in rows {
            let record = match kind.as_str() {
                "episode" => self.get_episode(&id)?.map(MemoryRecord::Episode),
                "entity" => self.get_entity(&id)?.map(MemoryRecord::Entity),
                "fact" => self.get_fact(&id)?.map(MemoryRecord::Fact),
                "edge" => self.get_edge(&id)?.map(MemoryRecord::Edge),
                _ => None,
            };
            if let Some(record) = record {
                records.push(record);
            }
        }

        sort_l3_records(&mut records);

        Ok(records)
    }

    pub fn increment_hit_counts(&self, memories: &[MemoryRecord]) -> Result<()> {
        if memories.is_empty() {
            return Ok(());
        }

        let mut episode_ids = Vec::new();
        let mut entity_ids = Vec::new();
        let mut fact_ids = Vec::new();
        let mut edge_ids = Vec::new();
        let mut seen = HashSet::new();

        for memory in memories {
            let key = format!("{}:{}", memory.kind(), memory.id());
            if !seen.insert(key) {
                continue;
            }

            match memory {
                MemoryRecord::Episode(_) => episode_ids.push(memory.id().to_string()),
                MemoryRecord::Entity(_) => entity_ids.push(memory.id().to_string()),
                MemoryRecord::Fact(_) => fact_ids.push(memory.id().to_string()),
                MemoryRecord::Edge(_) => edge_ids.push(memory.id().to_string()),
            }
        }

        let now = now_ts();
        let mut conn = self.conn.lock().expect("sqlite mutex poisoned");
        let transaction = conn.transaction()?;

        for id in &episode_ids {
            transaction.execute(
                "UPDATE episodes SET hit_count = hit_count + 1, last_seen_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
        }
        for id in &entity_ids {
            transaction.execute(
                "UPDATE entities SET hit_count = hit_count + 1, last_seen_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
        }
        for id in &fact_ids {
            transaction.execute(
                "UPDATE facts SET hit_count = hit_count + 1, updated_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
        }
        for id in &edge_ids {
            transaction.execute(
                "UPDATE edges SET hit_count = hit_count + 1, updated_at = ?2 WHERE id = ?1",
                params![id, now],
            )?;
        }

        transaction.commit()?;
        Ok(())
    }

    pub fn update_layer(&self, kind: &str, id: &str, layer: MemoryLayer) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let table = table_for_kind(kind)?;
        let now = now_ts();
        let sql = format!("UPDATE {} SET layer = ?2 WHERE id = ?1", table);
        conn.execute(&sql, params![id, layer.as_str()])?;
        conn.execute(
            "UPDATE memory_layers
             SET layer = ?2, last_promoted_at = ?3, updated_at = ?3
             WHERE memory_id = ?1 AND memory_kind = ?4",
            params![id, layer.as_str(), now, kind],
        )?;
        if matches!(kind, "episode" | "entity" | "fact") {
            queue_text_index_job(&conn, kind, id, IndexJobOperation::Upsert)?;
        }
        Ok(())
    }

    pub fn archive_record(&self, kind: &str, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let table = table_for_kind(kind)?;
        let now = now_ts();
        let sql = format!(
            "UPDATE {} SET archived_at = ?2, updated_at = ?2{} WHERE id = ?1",
            table,
            if matches!(kind, "fact" | "edge") {
                ", valid_to = ?2"
            } else {
                ""
            }
        );
        conn.execute(&sql, params![id, now])?;
        conn.execute(
            "UPDATE memory_layers
             SET status = 'archived', updated_at = ?2
             WHERE memory_id = ?1 AND memory_kind = ?3",
            params![id, now, kind],
        )?;
        queue_index_delete_jobs(&conn, kind, id)?;
        Ok(())
    }

    pub fn invalidate_record(&self, kind: &str, id: &str) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let table = table_for_kind(kind)?;
        let now = now_ts();
        let sql = format!(
            "UPDATE {} SET invalidated_at = ?2, updated_at = ?2{} WHERE id = ?1",
            table,
            if matches!(kind, "fact" | "edge") {
                ", valid_to = ?2"
            } else {
                ""
            }
        );
        conn.execute(&sql, params![id, now])?;
        conn.execute(
            "UPDATE memory_layers
             SET status = 'invalidated', updated_at = ?2
             WHERE memory_id = ?1 AND memory_kind = ?3",
            params![id, now, kind],
        )?;
        queue_index_delete_jobs(&conn, kind, id)?;
        Ok(())
    }

    pub fn record_index_ready(
        &self,
        name: &str,
        doc_count: usize,
        detail: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        record_index_ready(&conn, name, doc_count, detail)?;
        Ok(())
    }

    pub fn index_status(&self, name: &str) -> Result<IndexStatus> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut status = conn
            .query_row(
                "SELECT index_name, doc_count, status, detail
                 FROM index_state WHERE index_name = ?1 LIMIT 1",
                params![name],
                |row| {
                    Ok(IndexStatus {
                        name: row.get(0)?,
                        doc_count: row.get::<_, i64>(1)?.max(0) as usize,
                        status: row.get(2)?,
                        detail: row.get(3)?,
                        pending_updates: 0,
                        failed_updates: 0,
                        failed_attempts_max: 0,
                        last_error: None,
                    })
                },
            )
            .optional()?
            .unwrap_or(IndexStatus {
                name: name.to_string(),
                doc_count: 0,
                status: "unknown".to_string(),
                detail: None,
                pending_updates: 0,
                failed_updates: 0,
                failed_attempts_max: 0,
                last_error: None,
            });

        let (pending_updates, failed_updates, failed_attempts_max, last_error) =
            index_job_observability(&conn, name)?;
        status.pending_updates = pending_updates;
        status.failed_updates = failed_updates;
        status.failed_attempts_max = failed_attempts_max;
        status.last_error = last_error;
        if failed_updates > 0 {
            status.status = "failed".to_string();
            if status.detail.is_none() {
                status.detail = Some("restore failed for queued updates".to_string());
            }
        } else if pending_updates > 0 {
            status.status = "pending".to_string();
            if status.detail.is_none() {
                status.detail = Some("pending restore after queued updates".to_string());
            }
        }

        Ok(status)
    }

    pub fn duplicate_l1_episode_groups(&self) -> Result<Vec<Vec<String>>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT normalized_content
             FROM episodes
             WHERE layer = 'L1' AND archived_at IS NULL
             GROUP BY normalized_content
             HAVING COUNT(*) > 1",
        )?;
        let groups = stmt
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        drop(stmt);

        let mut result = Vec::new();
        for normalized_content in groups {
            let mut stmt = conn.prepare(
                "SELECT id FROM episodes
                 WHERE normalized_content = ?1 AND layer = 'L1' AND archived_at IS NULL
                 ORDER BY created_at ASC",
            )?;
            let ids = stmt
                .query_map(params![normalized_content], |row| row.get::<_, String>(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            result.push(ids);
        }
        Ok(result)
    }

    pub fn eligible_episode_ids_for_l2(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT id FROM episodes
             WHERE layer = 'L1' AND archived_at IS NULL AND invalidated_at IS NULL
               AND hit_count >= 1",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let ids = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }

    pub fn eligible_entity_ids_for_l2_by_support(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT e.id
             FROM entities e
             WHERE e.layer = 'L1'
               AND e.archived_at IS NULL
               AND e.invalidated_at IS NULL
               AND (
                    SELECT COUNT(DISTINCT support_scope_id)
                    FROM (
                        SELECT COALESCE(ep.session_id, ep.id) AS support_scope_id
                        FROM mentions m
                        JOIN episodes ep ON ep.id = m.episode_id
                        WHERE m.entity_id = e.id
                        UNION
                        SELECT COALESCE(ep.session_id, ep.id) AS support_scope_id
                        FROM facts f
                        JOIN episodes ep ON ep.id = f.source_episode_id
                        WHERE (f.subject_entity_id = e.id OR f.object_entity_id = e.id)
                          AND f.source_episode_id IS NOT NULL
                    )
               ) >= 2",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn eligible_fact_ids_for_l2_by_support(&self) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT f.id
            FROM facts f
             JOIN (
                SELECT LOWER(TRIM(subject_text)) AS subject_key,
                       LOWER(TRIM(predicate)) AS predicate_key,
                       LOWER(TRIM(object_text)) AS object_key
                FROM facts
                JOIN episodes ep ON ep.id = facts.source_episode_id
                WHERE facts.layer = 'L1'
                  AND facts.archived_at IS NULL
                  AND facts.invalidated_at IS NULL
                  AND facts.source_episode_id IS NOT NULL
                GROUP BY LOWER(TRIM(subject_text)), LOWER(TRIM(predicate)), LOWER(TRIM(object_text))
                HAVING COUNT(DISTINCT COALESCE(ep.session_id, ep.id)) >= 2
             ) supported
               ON LOWER(TRIM(f.subject_text)) = supported.subject_key
              AND LOWER(TRIM(f.predicate)) = supported.predicate_key
              AND LOWER(TRIM(f.object_text)) = supported.object_key
             WHERE f.layer = 'L1'
               AND f.archived_at IS NULL
               AND f.invalidated_at IS NULL",
        )?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn active_entity_ids_in_layers(&self, layers: &[MemoryLayer]) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let placeholders = (1..=layers.len())
            .map(|idx| format!("?{}", idx))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id FROM entities
             WHERE archived_at IS NULL
               AND invalidated_at IS NULL
               AND layer IN ({})",
            placeholders
        );
        let args = layers
            .iter()
            .map(|layer| layer.as_str())
            .collect::<Vec<_>>();
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn entity_support_scopes(&self, entity_id: &str) -> Result<Vec<(String, DateTime<Utc>)>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut stmt = conn.prepare(
            "SELECT support_scope_id, created_at
             FROM (
                SELECT COALESCE(ep.session_id, ep.id) AS support_scope_id,
                       ep.created_at AS created_at
                FROM mentions m
                JOIN episodes ep ON ep.id = m.episode_id
                WHERE m.entity_id = ?1
                UNION ALL
                SELECT COALESCE(ep.session_id, ep.id) AS support_scope_id,
                       ep.created_at AS created_at
                FROM facts f
                JOIN episodes ep ON ep.id = f.source_episode_id
                WHERE (f.subject_entity_id = ?1 OR f.object_entity_id = ?1)
                  AND f.source_episode_id IS NOT NULL
             )",
        )?;
        let rows = stmt.query_map(params![entity_id], |row| {
            Ok((row.get::<_, String>(0)?, ts_to_dt(row.get::<_, i64>(1)?)))
        })?;

        let mut earliest_by_scope = HashMap::<String, DateTime<Utc>>::new();
        for row in rows {
            let (scope_id, created_at) = row?;
            earliest_by_scope
                .entry(scope_id)
                .and_modify(|existing| {
                    if created_at < *existing {
                        *existing = created_at;
                    }
                })
                .or_insert(created_at);
        }

        Ok(earliest_by_scope.into_iter().collect())
    }

    pub fn related_ids_for_episode(&self, kind: &str, episode_id: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let sql = match kind {
            "entity" => {
                "SELECT id FROM entities
                 WHERE source_episode_id = ?1
                   AND layer = 'L1'
                   AND archived_at IS NULL
                   AND invalidated_at IS NULL"
            }
            "fact" => {
                "SELECT id FROM facts
                 WHERE source_episode_id = ?1
                   AND layer = 'L1'
                   AND archived_at IS NULL
                   AND invalidated_at IS NULL"
            }
            "edge" => {
                "SELECT id FROM edges
                 WHERE source_episode_id = ?1
                   AND layer = 'L1'
                   AND archived_at IS NULL
                   AND invalidated_at IS NULL"
            }
            _ => anyhow::bail!("unsupported related episode kind: {}", kind),
        };
        let mut stmt = conn.prepare(sql)?;
        let rows = stmt.query_map(params![episode_id], |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn active_facts_in_layers(&self, layers: &[MemoryLayer]) -> Result<Vec<FactRecord>> {
        if layers.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let placeholders = (1..=layers.len())
            .map(|index| format!("?{}", index))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id, subject_entity_id, subject_text, predicate, object_entity_id, object_text,
                    layer, confidence, source_episode_id, valid_from, valid_to, created_at, updated_at, archived_at, invalidated_at, hit_count
             FROM facts
             WHERE archived_at IS NULL
               AND invalidated_at IS NULL
               AND layer IN ({})",
            placeholders
        );
        let params = layers.iter().map(|layer| layer.as_str());
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params), map_fact)?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn matching_edge_ids(
        &self,
        subject_entity_id: &str,
        predicate: &str,
        object_entity_id: &str,
        layers: &[MemoryLayer],
    ) -> Result<Vec<String>> {
        if layers.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let placeholders = (4..=layers.len() + 3)
            .map(|index| format!("?{}", index))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id FROM edges
             WHERE subject_entity_id = ?1
               AND predicate = ?2
               AND object_entity_id = ?3
               AND archived_at IS NULL
               AND invalidated_at IS NULL
               AND layer IN ({})",
            placeholders
        );
        let mut args = vec![
            subject_entity_id.to_string(),
            predicate.to_string(),
            object_entity_id.to_string(),
        ];
        args.extend(layers.iter().map(|layer| layer.as_str().to_string()));
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn matching_edge_ids_for_source(
        &self,
        subject_entity_id: &str,
        predicate: &str,
        object_entity_id: &str,
        source_episode_id: &str,
        layers: &[MemoryLayer],
    ) -> Result<Vec<String>> {
        if layers.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let placeholders = (5..=layers.len() + 4)
            .map(|index| format!("?{}", index))
            .collect::<Vec<_>>()
            .join(",");
        let sql = format!(
            "SELECT id FROM edges
             WHERE subject_entity_id = ?1
               AND predicate = ?2
               AND object_entity_id = ?3
               AND source_episode_id = ?4
               AND archived_at IS NULL
               AND invalidated_at IS NULL
               AND layer IN ({})",
            placeholders
        );
        let mut args = vec![
            subject_entity_id.to_string(),
            predicate.to_string(),
            object_entity_id.to_string(),
            source_episode_id.to_string(),
        ];
        args.extend(layers.iter().map(|layer| layer.as_str().to_string()));
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(args.iter()), |row| row.get(0))?;
        Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
    }

    pub fn support_scope_for_episode(
        &self,
        episode_id: &str,
    ) -> Result<Option<(String, DateTime<Utc>)>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.query_row(
            "SELECT COALESCE(session_id, id), created_at FROM episodes WHERE id = ?1 LIMIT 1",
            params![episode_id],
            |row| Ok((row.get::<_, String>(0)?, ts_to_dt(row.get::<_, i64>(1)?))),
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn eligible_ids_for_l3(&self, kind: &str) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let table = table_for_kind(kind)?;
        let sql = format!(
            "SELECT id FROM {}
             WHERE layer = 'L2' AND archived_at IS NULL AND invalidated_at IS NULL
               AND hit_count >= 2",
            table
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        let ids = rows.collect::<rusqlite::Result<Vec<_>>>()?;
        Ok(ids)
    }

    pub fn stats(&self) -> Result<(usize, usize, usize, usize)> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let episode_count = count_table(&conn, "episodes")?;
        let entity_count = count_table(&conn, "entities")?;
        let fact_count = count_table(&conn, "facts")?;
        let edge_count = count_table(&conn, "edges")?;
        Ok((episode_count, entity_count, fact_count, edge_count))
    }

    pub fn layer_summary(&self) -> Result<LayerSummary> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut summary = LayerSummary::default();
        let mut stmt = conn.prepare(
            "SELECT layer, status, COUNT(*)
             FROM memory_layers
             GROUP BY layer, status",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?.max(0) as usize,
            ))
        })?;

        for row in rows {
            let (layer, status, count) = row?;
            match status.as_str() {
                "active" => match layer.as_str() {
                    "L1" => summary.l1 += count,
                    "L2" => summary.l2 += count,
                    "L3" => summary.l3 += count,
                    _ => {}
                },
                "archived" => summary.archived += count,
                "invalidated" => summary.invalidated += count,
                _ => {}
            }
        }

        Ok(summary)
    }
}

fn load_search_document_record(
    conn: &Connection,
    kind: &str,
    id: &str,
) -> Result<Option<(String, String, String, String)>> {
    match kind {
        "episode" => conn
            .query_row(
                "SELECT id, layer, content
                 FROM episodes
                 WHERE id = ?1 AND archived_at IS NULL AND invalidated_at IS NULL
                 LIMIT 1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        "episode".to_string(),
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(Into::into),
        "entity" => {
            let row = conn
                .query_row(
                    "SELECT id, layer, canonical_name
                     FROM entities
                     WHERE id = ?1 AND archived_at IS NULL AND invalidated_at IS NULL
                     LIMIT 1",
                    params![id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, String>(2)?,
                        ))
                    },
                )
                .optional()?;
            let Some((id, layer, mut body)) = row else {
                return Ok(None);
            };
            let aliases = load_aliases(conn, &id)?;
            if !aliases.is_empty() {
                body.push(' ');
                body.push_str(&aliases.join(" "));
            }
            Ok(Some((id, "entity".to_string(), layer, body)))
        }
        "fact" => conn
            .query_row(
                "SELECT id, layer, subject_text, predicate, object_text
                 FROM facts
                 WHERE id = ?1 AND archived_at IS NULL AND invalidated_at IS NULL
                 LIMIT 1",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        "fact".to_string(),
                        row.get::<_, String>(1)?,
                        format!(
                            "{} {} {}",
                            row.get::<_, String>(2)?,
                            row.get::<_, String>(3)?,
                            row.get::<_, String>(4)?
                        ),
                    ))
                },
            )
            .optional()
            .map_err(Into::into),
        other => anyhow::bail!("unsupported search document kind: {}", other),
    }
}

fn find_active_entity_id_by_reference(
    conn: &Connection,
    normalized_name: &str,
) -> Result<Option<String>> {
    conn.query_row(
        "SELECT e.id
         FROM entities e
         LEFT JOIN entity_aliases a ON a.entity_id = e.id
         WHERE e.archived_at IS NULL
           AND e.invalidated_at IS NULL
           AND (e.normalized_name = ?1 OR a.normalized_alias = ?1)
         ORDER BY CASE WHEN e.normalized_name = ?1 THEN 0 ELSE 1 END, e.created_at ASC
         LIMIT 1",
        params![normalized_name],
        |row| row.get::<_, String>(0),
    )
    .optional()
    .map_err(Into::into)
}

fn load_vector_document_record(
    conn: &Connection,
    kind: &str,
    id: &str,
) -> Result<Option<(String, String, Vec<f32>)>> {
    let raw = match kind {
        "episode" => conn
            .query_row(
                "SELECT vector_json
                 FROM episodes
                 WHERE id = ?1 AND archived_at IS NULL AND invalidated_at IS NULL
                   AND vector_json IS NOT NULL
                 LIMIT 1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        "entity" => conn
            .query_row(
                "SELECT vector_json
                 FROM entities
                 WHERE id = ?1 AND archived_at IS NULL AND invalidated_at IS NULL
                   AND vector_json IS NOT NULL
                 LIMIT 1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        "fact" => conn
            .query_row(
                "SELECT vector_json
                 FROM facts
                 WHERE id = ?1 AND archived_at IS NULL AND invalidated_at IS NULL
                   AND vector_json IS NOT NULL
                 LIMIT 1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?,
        other => anyhow::bail!("unsupported vector document kind: {}", other),
    };

    raw.map(|payload| Ok((id.to_string(), kind.to_string(), json_to_vec(&payload)?)))
        .transpose()
}

fn queue_text_index_job(
    conn: &Connection,
    memory_kind: &str,
    memory_id: &str,
    operation: IndexJobOperation,
) -> Result<()> {
    queue_index_job(conn, "text", memory_kind, memory_id, operation)
}

fn queue_vector_index_job(
    conn: &Connection,
    memory_kind: &str,
    memory_id: &str,
    operation: IndexJobOperation,
) -> Result<()> {
    queue_index_job(conn, "vector", memory_kind, memory_id, operation)
}

fn memory_key(kind: &str, id: &str) -> String {
    format!("{}:{}", kind, id)
}

fn sort_l3_records(records: &mut [MemoryRecord]) {
    records.sort_by(|left, right| {
        right
            .hit_count()
            .cmp(&left.hit_count())
            .then_with(|| right.activity_at().cmp(&left.activity_at()))
            .then_with(|| left.id().cmp(right.id()))
    });
}

fn queue_index_job(
    conn: &Connection,
    index_name: &str,
    memory_kind: &str,
    memory_id: &str,
    operation: IndexJobOperation,
) -> Result<()> {
    let now = now_ts();
    conn.execute(
        "INSERT INTO index_jobs
         (id, index_name, memory_kind, memory_id, operation, status, attempts, last_error, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, 0, NULL, ?7, ?7)
         ON CONFLICT(index_name, memory_kind, memory_id) DO UPDATE SET
             operation = excluded.operation,
             status = excluded.status,
             attempts = 0,
             last_error = NULL,
             updated_at = excluded.updated_at",
        params![
            Uuid::new_v4().to_string(),
            index_name,
            memory_kind,
            memory_id,
            operation.as_str(),
            IndexJobStatus::Pending.as_str(),
            now,
        ],
    )?;
    mark_index_pending(
        conn,
        index_name,
        Some("pending restore after queued updates"),
    )?;
    Ok(())
}

fn queue_index_delete_jobs(conn: &Connection, kind: &str, id: &str) -> Result<()> {
    match kind {
        "episode" | "entity" | "fact" => {
            queue_text_index_job(conn, kind, id, IndexJobOperation::Delete)?;
            queue_vector_index_job(conn, kind, id, IndexJobOperation::Delete)?;
        }
        "edge" => {}
        other => anyhow::bail!("unsupported memory kind: {}", other),
    }
    Ok(())
}

fn clear_index_jobs_by_ids(conn: &Connection, index_name: &str, job_ids: &[String]) -> Result<()> {
    let placeholders = (1..=job_ids.len())
        .map(|index| format!("?{}", index))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "DELETE FROM index_jobs WHERE index_name = ?{} AND id IN ({})",
        job_ids.len() + 1,
        placeholders
    );
    let mut args = job_ids.to_vec();
    args.push(index_name.to_string());
    conn.execute(&sql, rusqlite::params_from_iter(args.iter()))?;
    Ok(())
}

fn fail_index_jobs_by_ids(
    conn: &Connection,
    index_name: &str,
    job_ids: &[String],
    detail: &str,
) -> Result<()> {
    let placeholders = (1..=job_ids.len())
        .map(|index| format!("?{}", index))
        .collect::<Vec<_>>()
        .join(",");
    let sql = format!(
        "UPDATE index_jobs
         SET status = ?{}, attempts = attempts + 1, last_error = ?{}, updated_at = ?{}
         WHERE index_name = ?{} AND id IN ({})",
        job_ids.len() + 1,
        job_ids.len() + 2,
        job_ids.len() + 3,
        job_ids.len() + 4,
        placeholders
    );
    let mut args = job_ids.to_vec();
    args.push(IndexJobStatus::Failed.as_str().to_string());
    args.push(detail.to_string());
    args.push(now_ts().to_string());
    args.push(index_name.to_string());
    conn.execute(&sql, rusqlite::params_from_iter(args.iter()))?;
    mark_index_failed(conn, index_name, Some(detail))?;
    Ok(())
}

fn index_job_observability(
    conn: &Connection,
    index_name: &str,
) -> Result<(usize, usize, usize, Option<String>)> {
    let mut stmt = conn.prepare(
        "SELECT status, COUNT(*)
         FROM index_jobs
         WHERE index_name = ?1
         GROUP BY status",
    )?;
    let rows = stmt.query_map(params![index_name], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?.max(0) as usize,
        ))
    })?;
    let mut pending = 0;
    let mut failed = 0;
    for row in rows {
        let (status, count) = row?;
        match status.as_str() {
            "pending" => pending = count,
            "failed" => failed = count,
            _ => {}
        }
    }

    let failed_attempts_max = conn.query_row(
        "SELECT COALESCE(MAX(attempts), 0)
         FROM index_jobs
         WHERE index_name = ?1
           AND status = 'failed'",
        params![index_name],
        |row| Ok(row.get::<_, i64>(0)?.max(0) as usize),
    )?;
    let last_error = conn
        .query_row(
            "SELECT last_error
             FROM index_jobs
             WHERE index_name = ?1
               AND status = 'failed'
             ORDER BY updated_at DESC, created_at DESC
             LIMIT 1",
            params![index_name],
            |row| row.get::<_, Option<String>>(0),
        )
        .optional()?
        .flatten();

    Ok((pending, failed, failed_attempts_max, last_error))
}

fn record_index_ready(
    conn: &Connection,
    name: &str,
    doc_count: usize,
    detail: Option<&str>,
) -> Result<()> {
    conn.execute(
        "INSERT INTO index_state (index_name, doc_count, status, detail, last_rebuilt_at)
         VALUES (?1, ?2, 'ready', ?3, ?4)
         ON CONFLICT(index_name) DO UPDATE SET
             doc_count = excluded.doc_count,
             status = excluded.status,
             detail = excluded.detail,
             last_rebuilt_at = excluded.last_rebuilt_at",
        params![name, doc_count as i64, detail, now_ts()],
    )?;
    Ok(())
}

fn mark_index_pending(conn: &Connection, name: &str, detail: Option<&str>) -> Result<()> {
    let doc_count = current_index_doc_count(conn, name)?;
    conn.execute(
        "INSERT INTO index_state (index_name, doc_count, status, detail, last_rebuilt_at)
         VALUES (?1, ?2, 'pending', ?3, NULL)
         ON CONFLICT(index_name) DO UPDATE SET
             doc_count = excluded.doc_count,
             status = excluded.status,
             detail = excluded.detail",
        params![name, doc_count as i64, detail],
    )?;
    Ok(())
}

fn mark_index_failed(conn: &Connection, name: &str, detail: Option<&str>) -> Result<()> {
    let doc_count = current_index_doc_count(conn, name)?;
    conn.execute(
        "INSERT INTO index_state (index_name, doc_count, status, detail, last_rebuilt_at)
         VALUES (?1, ?2, 'failed', ?3, NULL)
         ON CONFLICT(index_name) DO UPDATE SET
             doc_count = excluded.doc_count,
             status = excluded.status,
             detail = excluded.detail",
        params![name, doc_count as i64, detail],
    )?;
    Ok(())
}

fn current_index_doc_count(conn: &Connection, name: &str) -> Result<usize> {
    let value = conn
        .query_row(
            "SELECT doc_count FROM index_state WHERE index_name = ?1 LIMIT 1",
            params![name],
            |row| row.get::<_, i64>(0),
        )
        .optional()?
        .unwrap_or(0);
    Ok(value.max(0) as usize)
}

fn to_sql_error(error: anyhow::Error) -> rusqlite::Error {
    rusqlite::Error::FromSqlConversionFailure(
        0,
        rusqlite::types::Type::Text,
        Box::new(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            error.to_string(),
        )),
    )
}

fn init_schema(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS episodes (
            id TEXT PRIMARY KEY,
            content TEXT NOT NULL,
            normalized_content TEXT NOT NULL,
            layer TEXT NOT NULL,
            confidence REAL NOT NULL,
            source_episode_id TEXT NULL,
            session_id TEXT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_seen_at INTEGER NOT NULL,
            archived_at INTEGER NULL,
            invalidated_at INTEGER NULL,
            hit_count INTEGER NOT NULL DEFAULT 0,
            structured_at INTEGER NULL,
            vector_json TEXT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_episodes_normalized ON episodes(normalized_content);
        CREATE INDEX IF NOT EXISTS idx_episodes_layer ON episodes(layer);

        CREATE TABLE IF NOT EXISTS entities (
            id TEXT PRIMARY KEY,
            entity_type TEXT NOT NULL,
            canonical_name TEXT NOT NULL,
            normalized_name TEXT NOT NULL,
            confidence REAL NOT NULL,
            source_episode_id TEXT NULL,
            layer TEXT NOT NULL DEFAULT 'L1',
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            last_seen_at INTEGER NOT NULL,
            archived_at INTEGER NULL,
            invalidated_at INTEGER NULL,
            hit_count INTEGER NOT NULL DEFAULT 0,
            vector_json TEXT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_entities_normalized ON entities(normalized_name);

        CREATE TABLE IF NOT EXISTS entity_aliases (
            id TEXT PRIMARY KEY,
            entity_id TEXT NOT NULL,
            alias TEXT NOT NULL,
            normalized_alias TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            UNIQUE(entity_id, normalized_alias)
        );
        CREATE INDEX IF NOT EXISTS idx_entity_aliases_normalized ON entity_aliases(normalized_alias);

        CREATE TABLE IF NOT EXISTS mentions (
            id TEXT PRIMARY KEY,
            episode_id TEXT NOT NULL,
            entity_id TEXT NOT NULL,
            role TEXT NOT NULL,
            confidence REAL NOT NULL,
            created_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS facts (
            id TEXT PRIMARY KEY,
            subject_entity_id TEXT NULL,
            subject_text TEXT NOT NULL,
            predicate TEXT NOT NULL,
            object_entity_id TEXT NULL,
            object_text TEXT NOT NULL,
            confidence REAL NOT NULL,
            source_episode_id TEXT NULL,
            layer TEXT NOT NULL,
            valid_from INTEGER NULL,
            valid_to INTEGER NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            archived_at INTEGER NULL,
            invalidated_at INTEGER NULL,
            hit_count INTEGER NOT NULL DEFAULT 0,
            vector_json TEXT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_facts_layer ON facts(layer);

        CREATE TABLE IF NOT EXISTS edges (
            id TEXT PRIMARY KEY,
            subject_entity_id TEXT NOT NULL,
            predicate TEXT NOT NULL,
            object_entity_id TEXT NOT NULL,
            weight REAL NOT NULL,
            source_episode_id TEXT NULL,
            layer TEXT NOT NULL,
            valid_from INTEGER NULL,
            valid_to INTEGER NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            archived_at INTEGER NULL,
            invalidated_at INTEGER NULL,
            hit_count INTEGER NOT NULL DEFAULT 0
        );
        CREATE INDEX IF NOT EXISTS idx_edges_subject ON edges(subject_entity_id);
        CREATE INDEX IF NOT EXISTS idx_edges_object ON edges(object_entity_id);

        CREATE TABLE IF NOT EXISTS memory_layers (
            memory_id TEXT NOT NULL,
            memory_kind TEXT NOT NULL,
            layer TEXT NOT NULL,
            status TEXT NOT NULL,
            last_promoted_at INTEGER NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            PRIMARY KEY(memory_id, memory_kind)
        );

        CREATE TABLE IF NOT EXISTS index_state (
            index_name TEXT PRIMARY KEY,
            doc_count INTEGER NOT NULL,
            status TEXT NOT NULL,
            detail TEXT NULL,
            last_rebuilt_at INTEGER NULL
        );

        CREATE TABLE IF NOT EXISTS index_jobs (
            id TEXT PRIMARY KEY,
            index_name TEXT NOT NULL,
            memory_kind TEXT NOT NULL,
            memory_id TEXT NOT NULL,
            operation TEXT NOT NULL,
            status TEXT NOT NULL,
            attempts INTEGER NOT NULL DEFAULT 0,
            last_error TEXT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL,
            UNIQUE(index_name, memory_kind, memory_id)
        );

        DROP TABLE IF EXISTS dream_jobs;
        "#,
    )?;
    ensure_column(conn, "facts", "valid_from", "INTEGER NULL")?;
    ensure_column(conn, "facts", "valid_to", "INTEGER NULL")?;
    ensure_column(conn, "edges", "valid_from", "INTEGER NULL")?;
    ensure_column(conn, "edges", "valid_to", "INTEGER NULL")?;
    ensure_column(conn, "episodes", "session_id", "TEXT NULL")?;
    ensure_column(conn, "episodes", "structured_at", "INTEGER NULL")?;
    Ok(())
}

fn map_episode(row: &rusqlite::Row<'_>) -> rusqlite::Result<EpisodeRecord> {
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

fn map_fact(row: &rusqlite::Row<'_>) -> rusqlite::Result<FactRecord> {
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

fn map_edge(row: &rusqlite::Row<'_>) -> rusqlite::Result<EdgeRecord> {
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

fn load_aliases(conn: &Connection, entity_id: &str) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT alias FROM entity_aliases WHERE entity_id = ?1 ORDER BY alias ASC")?;
    let rows = stmt.query_map(params![entity_id], |row| row.get(0))?;
    let aliases = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(aliases)
}

fn count_table(conn: &Connection, table: &str) -> Result<usize> {
    let sql = format!("SELECT COUNT(*) FROM {}", table);
    conn.query_row(&sql, [], |row| row.get::<_, i64>(0))
        .map(|count| count.max(0) as usize)
        .map_err(Into::into)
}

fn ensure_column(conn: &Connection, table: &str, column: &str, definition: &str) -> Result<()> {
    let pragma = format!("PRAGMA table_info({})", table);
    let mut stmt = conn.prepare(&pragma)?;
    let exists = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .collect::<rusqlite::Result<Vec<_>>>()?
        .into_iter()
        .any(|name| name == column);
    drop(stmt);

    if !exists {
        let sql = format!("ALTER TABLE {} ADD COLUMN {} {}", table, column, definition);
        conn.execute(&sql, [])?;
    }

    Ok(())
}

fn table_for_kind(kind: &str) -> Result<&'static str> {
    match kind {
        "episode" => Ok("episodes"),
        "entity" => Ok("entities"),
        "fact" => Ok("facts"),
        "edge" => Ok("edges"),
        _ => anyhow::bail!("unsupported memory kind: {}", kind),
    }
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

pub fn normalize_text(text: &str) -> String {
    text.to_ascii_lowercase()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn now_ts() -> i64 {
    Utc::now().timestamp_millis()
}

fn ts_to_dt(ts: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp_millis(ts).unwrap_or_else(Utc::now)
}

fn vec_to_json(vector: &[f32]) -> Result<String> {
    Ok(serde_json::to_string(vector)?)
}

fn json_to_vec(raw: &str) -> Result<Vec<f32>> {
    let value: Value = serde_json::from_str(raw)?;
    let array = value
        .as_array()
        .context("vector json is not an array")?
        .iter()
        .map(|item| item.as_f64().unwrap_or_default() as f32)
        .collect();
    Ok(array)
}

#[cfg(test)]
mod tests {
    use super::{Database, ObservationContext};
    use anyhow::Result;
    use chrono::{TimeZone, Utc};
    use rusqlite::{Connection, OptionalExtension};
    use tempfile::TempDir;

    use crate::types::{EntityInput, EpisodeInput, ExtractionSource, MemoryLayer, MemoryRecord};

    fn dt_to_ts(dt: chrono::DateTime<Utc>) -> i64 {
        dt.timestamp_millis()
    }

    #[test]
    fn open_migrates_fact_and_edge_validity_columns() -> Result<()> {
        let temp = TempDir::new()?;
        let db_path = temp.path().join("memory.db");

        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE facts (
                id TEXT PRIMARY KEY,
                subject_entity_id TEXT NULL,
                subject_text TEXT NOT NULL,
                predicate TEXT NOT NULL,
                object_entity_id TEXT NULL,
                object_text TEXT NOT NULL,
                confidence REAL NOT NULL,
                source_episode_id TEXT NULL,
                layer TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                archived_at INTEGER NULL,
                invalidated_at INTEGER NULL,
                hit_count INTEGER NOT NULL DEFAULT 0,
                vector_json TEXT NULL
            );
            CREATE TABLE edges (
                id TEXT PRIMARY KEY,
                subject_entity_id TEXT NOT NULL,
                predicate TEXT NOT NULL,
                object_entity_id TEXT NOT NULL,
                weight REAL NOT NULL,
                source_episode_id TEXT NULL,
                layer TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                archived_at INTEGER NULL,
                invalidated_at INTEGER NULL,
                hit_count INTEGER NOT NULL DEFAULT 0
            );
            CREATE TABLE episodes (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                normalized_content TEXT NOT NULL,
                layer TEXT NOT NULL,
                confidence REAL NOT NULL,
                source_episode_id TEXT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL,
                archived_at INTEGER NULL,
                invalidated_at INTEGER NULL,
                hit_count INTEGER NOT NULL DEFAULT 0,
                vector_json TEXT NULL
            );
            CREATE TABLE entities (
                id TEXT PRIMARY KEY,
                entity_type TEXT NOT NULL,
                canonical_name TEXT NOT NULL,
                normalized_name TEXT NOT NULL,
                confidence REAL NOT NULL,
                source_episode_id TEXT NULL,
                layer TEXT NOT NULL DEFAULT 'L1',
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                last_seen_at INTEGER NOT NULL,
                archived_at INTEGER NULL,
                invalidated_at INTEGER NULL,
                hit_count INTEGER NOT NULL DEFAULT 0,
                vector_json TEXT NULL
            );
            CREATE TABLE entity_aliases (
                id TEXT PRIMARY KEY,
                entity_id TEXT NOT NULL,
                alias TEXT NOT NULL,
                normalized_alias TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                UNIQUE(entity_id, normalized_alias)
            );
            CREATE TABLE mentions (
                id TEXT PRIMARY KEY,
                episode_id TEXT NOT NULL,
                entity_id TEXT NOT NULL,
                role TEXT NOT NULL,
                confidence REAL NOT NULL,
                created_at INTEGER NOT NULL
            );
            CREATE TABLE memory_layers (
                memory_id TEXT NOT NULL,
                memory_kind TEXT NOT NULL,
                layer TEXT NOT NULL,
                status TEXT NOT NULL,
                last_promoted_at INTEGER NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL,
                PRIMARY KEY(memory_id, memory_kind)
            );
            CREATE TABLE index_state (
                index_name TEXT PRIMARY KEY,
                doc_count INTEGER NOT NULL,
                status TEXT NOT NULL,
                detail TEXT NULL,
                last_rebuilt_at INTEGER NULL
            );
            "#,
        )?;
        drop(conn);

        let _db = Database::open(&db_path)?;
        let conn = Connection::open(&db_path)?;

        for (table, column) in [
            ("episodes", "session_id"),
            ("facts", "valid_from"),
            ("facts", "valid_to"),
            ("edges", "valid_from"),
            ("edges", "valid_to"),
        ] {
            let pragma = format!("PRAGMA table_info({})", table);
            let names = conn
                .prepare(&pragma)?
                .query_map([], |row| row.get::<_, String>(1))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            assert!(
                names.iter().any(|name| name == column),
                "expected column {}.{} to be migrated",
                table,
                column
            );
        }

        Ok(())
    }

    #[test]
    fn open_drops_obsolete_dream_jobs_table() -> Result<()> {
        let temp = TempDir::new()?;
        let db_path = temp.path().join("memory.db");

        let conn = Connection::open(&db_path)?;
        conn.execute_batch(
            r#"
            CREATE TABLE dream_jobs (
                id TEXT PRIMARY KEY,
                trigger TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            );
            "#,
        )?;
        drop(conn);

        let _db = Database::open(&db_path)?;
        let conn = Connection::open(&db_path)?;
        let exists = conn
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'dream_jobs' LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        assert!(
            exists.is_none(),
            "expected obsolete dream_jobs table to be dropped"
        );

        Ok(())
    }

    #[test]
    fn increment_hit_counts_updates_multiple_records_in_one_call() -> Result<()> {
        let temp = TempDir::new()?;
        let db = Database::open(&temp.path().join("memory.db"))?;

        let first = db.insert_episode(
            &EpisodeInput {
                content: "alpha".to_string(),
                layer: MemoryLayer::L1,
                entities: Vec::new(),
                facts: Vec::new(),
                source_episode_id: None,
                session_id: None,
                recorded_at: None,
                confidence: 0.9,
            },
            None,
        )?;
        let second = db.insert_episode(
            &EpisodeInput {
                content: "beta".to_string(),
                layer: MemoryLayer::L1,
                entities: Vec::new(),
                facts: Vec::new(),
                source_episode_id: None,
                session_id: None,
                recorded_at: None,
                confidence: 0.9,
            },
            None,
        )?;

        let first_before = first.last_seen_at;
        let second_before = second.last_seen_at;
        db.increment_hit_counts(&[
            MemoryRecord::Episode(first.clone()),
            MemoryRecord::Episode(second.clone()),
        ])?;

        let first_after = db
            .get_episode(&first.id)?
            .expect("expected first episode after batch hit update");
        let second_after = db
            .get_episode(&second.id)?
            .expect("expected second episode after batch hit update");

        assert_eq!(first_after.hit_count, 1);
        assert_eq!(second_after.hit_count, 1);
        assert!(first_after.last_seen_at >= first_before);
        assert!(second_after.last_seen_at >= second_before);
        Ok(())
    }

    #[test]
    fn load_l3_records_prioritizes_hit_count_then_activity() -> Result<()> {
        let temp = TempDir::new()?;
        let db = Database::open(&temp.path().join("memory.db"))?;

        let alpha = db.insert_episode(
            &EpisodeInput {
                content: "alpha".to_string(),
                layer: MemoryLayer::L3,
                entities: Vec::new(),
                facts: Vec::new(),
                source_episode_id: None,
                session_id: None,
                recorded_at: None,
                confidence: 0.9,
            },
            None,
        )?;
        let beta = db.insert_episode(
            &EpisodeInput {
                content: "beta".to_string(),
                layer: MemoryLayer::L3,
                entities: Vec::new(),
                facts: Vec::new(),
                source_episode_id: None,
                session_id: None,
                recorded_at: None,
                confidence: 0.9,
            },
            None,
        )?;
        let gamma = db.insert_episode(
            &EpisodeInput {
                content: "gamma".to_string(),
                layer: MemoryLayer::L3,
                entities: Vec::new(),
                facts: Vec::new(),
                source_episode_id: None,
                session_id: None,
                recorded_at: None,
                confidence: 0.9,
            },
            None,
        )?;

        {
            let conn = db.conn.lock().expect("sqlite mutex poisoned");
            conn.execute(
                "UPDATE episodes SET hit_count = ?2, last_seen_at = ?3 WHERE id = ?1",
                rusqlite::params![
                    alpha.id,
                    5_i64,
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap())
                ],
            )?;
            conn.execute(
                "UPDATE episodes SET hit_count = ?2, last_seen_at = ?3 WHERE id = ?1",
                rusqlite::params![
                    beta.id,
                    3_i64,
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 11, 0, 0).unwrap())
                ],
            )?;
            conn.execute(
                "UPDATE memory_layers SET updated_at = ?2 WHERE memory_id = ?1 AND memory_kind = 'episode'",
                rusqlite::params![
                    alpha.id,
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 9, 0, 0).unwrap())
                ],
            )?;
            conn.execute(
                "UPDATE memory_layers SET updated_at = ?2 WHERE memory_id = ?1 AND memory_kind = 'episode'",
                rusqlite::params![
                    beta.id,
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 11, 30, 0).unwrap())
                ],
            )?;
            conn.execute(
                "UPDATE episodes SET hit_count = ?2, last_seen_at = ?3 WHERE id = ?1",
                rusqlite::params![
                    gamma.id,
                    1_i64,
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 8, 0, 0).unwrap())
                ],
            )?;
            conn.execute(
                "UPDATE memory_layers SET updated_at = ?2 WHERE memory_id = ?1 AND memory_kind = 'episode'",
                rusqlite::params![
                    gamma.id,
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 12, 0, 0).unwrap())
                ],
            )?;
        }

        let first = db.load_l3_records(1)?;
        assert!(matches!(
            first.first(),
            Some(MemoryRecord::Episode(record)) if record.id == alpha.id
        ));

        {
            let conn = db.conn.lock().expect("sqlite mutex poisoned");
            conn.execute(
                "UPDATE episodes SET hit_count = ?2, last_seen_at = ?3 WHERE id = ?1",
                rusqlite::params![
                    beta.id,
                    5_i64,
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 12, 0, 0).unwrap())
                ],
            )?;
        }

        let first = db.load_l3_records(1)?;
        assert!(matches!(
            first.first(),
            Some(MemoryRecord::Episode(record)) if record.id == beta.id
        ));
        Ok(())
    }

    #[test]
    fn related_graph_records_applies_limit_per_hop_after_dedup() -> Result<()> {
        let temp = TempDir::new()?;
        let db = Database::open(&temp.path().join("memory.db"))?;
        let observed_at = Utc.with_ymd_and_hms(2026, 4, 22, 12, 0, 0).unwrap();

        let episode = db.insert_episode(
            &EpisodeInput {
                content: "graph expansion".to_string(),
                layer: MemoryLayer::L1,
                entities: Vec::new(),
                facts: Vec::new(),
                source_episode_id: None,
                session_id: None,
                recorded_at: Some(observed_at),
                confidence: 0.9,
            },
            None,
        )?;

        let alice = db.upsert_entity(
            &EntityInput {
                entity_type: "person".to_string(),
                name: "Alice".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
            MemoryLayer::L1,
            ObservationContext {
                source_episode_id: Some(&episode.id),
                observed_at,
            },
            None,
        )?;
        let paris = db.upsert_entity(
            &EntityInput {
                entity_type: "place".to_string(),
                name: "Paris".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
            MemoryLayer::L1,
            ObservationContext {
                source_episode_id: Some(&episode.id),
                observed_at,
            },
            None,
        )?;
        let france = db.upsert_entity(
            &EntityInput {
                entity_type: "place".to_string(),
                name: "France".to_string(),
                aliases: Vec::new(),
                confidence: 0.95,
                source: ExtractionSource::Manual,
            },
            MemoryLayer::L1,
            ObservationContext {
                source_episode_id: Some(&episode.id),
                observed_at,
            },
            None,
        )?;

        let hop1_edge = db.insert_edge(
            &alice.id,
            "lives_in",
            &paris.id,
            0.95,
            MemoryLayer::L1,
            ObservationContext {
                source_episode_id: Some(&episode.id),
                observed_at,
            },
        )?;
        let hop2_edge = db.insert_edge(
            &paris.id,
            "located_in",
            &france.id,
            0.95,
            MemoryLayer::L1,
            ObservationContext {
                source_episode_id: Some(&episode.id),
                observed_at,
            },
        )?;

        let records = db.related_graph_records(&[alice.id.clone()], 2, 1)?;

        assert_eq!(records.len(), 2, "expected one unique record from each hop");
        assert!(records.iter().any(
            |(record, hop)| matches!(record, MemoryRecord::Edge(edge) if *hop == 1 && edge.id == hop1_edge.id)
        ));
        assert!(records.iter().any(
            |(record, hop)| matches!(record, MemoryRecord::Edge(edge) if *hop == 2 && edge.id == hop2_edge.id)
        ));
        Ok(())
    }

    #[test]
    fn index_status_exposes_failed_attempts_and_latest_error() -> Result<()> {
        let temp = TempDir::new()?;
        let db = Database::open(&temp.path().join("memory.db"))?;

        {
            let conn = db.conn.lock().expect("sqlite mutex poisoned");
            conn.execute(
                "INSERT INTO index_jobs
                 (id, index_name, memory_kind, memory_id, operation, status, attempts, last_error, created_at, updated_at)
                 VALUES (?1, 'vector', 'episode', 'ep-1', 'upsert', 'failed', 2, ?2, ?3, ?3)",
                rusqlite::params![
                    "job-1",
                    "first vector failure",
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap())
                ],
            )?;
            conn.execute(
                "INSERT INTO index_jobs
                 (id, index_name, memory_kind, memory_id, operation, status, attempts, last_error, created_at, updated_at)
                 VALUES (?1, 'vector', 'episode', 'ep-2', 'upsert', 'failed', 3, ?2, ?3, ?4)",
                rusqlite::params![
                    "job-2",
                    "latest vector failure",
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 10, 0, 0).unwrap()),
                    dt_to_ts(Utc.with_ymd_and_hms(2026, 4, 22, 11, 0, 0).unwrap())
                ],
            )?;
        }

        let status = db.index_status("vector")?;
        assert_eq!(status.status, "failed");
        assert_eq!(status.failed_updates, 2);
        assert_eq!(status.failed_attempts_max, 3);
        assert_eq!(status.last_error.as_deref(), Some("latest vector failure"));
        Ok(())
    }
}

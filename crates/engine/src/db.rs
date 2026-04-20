use std::{collections::HashSet, sync::Mutex};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

use crate::types::{
    EdgeRecord, EntityInput, EntityRecord, EpisodeInput, EpisodeRecord, FactInput, FactRecord,
    IndexStatus, MemoryLayer, MemoryRecord,
};

pub struct Database {
    conn: Mutex<Connection>,
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
        let now = now_ts();
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

        drop(conn);
        self.get_episode(&id)?
            .context("failed to load inserted episode")
    }

    pub fn upsert_entity(
        &self,
        input: &EntityInput,
        layer: MemoryLayer,
        source_episode_id: Option<&str>,
        vector: Option<&[f32]>,
    ) -> Result<EntityRecord> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let now = now_ts();
        let normalized_name = normalize_text(&input.name);
        let existing_id: Option<String> = conn
            .query_row(
                "SELECT id FROM entities WHERE normalized_name = ?1 AND archived_at IS NULL LIMIT 1",
                params![normalized_name],
                |row| row.get(0),
            )
            .optional()?;

        let entity_id = if let Some(existing_id) = existing_id {
            conn.execute(
                "UPDATE entities
                 SET confidence = MAX(confidence, ?2), updated_at = ?3, last_seen_at = ?3
                 WHERE id = ?1",
                params![existing_id, input.confidence, now],
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
                    source_episode_id,
                    now,
                    vector_json
                ],
            )?;
            conn.execute(
                "INSERT INTO memory_layers (memory_id, memory_kind, layer, status, created_at, updated_at)
                 VALUES (?1, 'entity', ?2, 'active', ?3, ?3)",
                params![entity_id, layer.as_str(), now],
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
                    now
                ],
            )?;
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

    pub fn insert_fact(
        &self,
        input: &FactInput,
        layer: MemoryLayer,
        source_episode_id: Option<&str>,
        subject_entity_id: Option<&str>,
        object_entity_id: Option<&str>,
        vector: Option<&[f32]>,
    ) -> Result<FactRecord> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let now = now_ts();
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
                source_episode_id,
                layer.as_str(),
                now,
                vector_json
            ],
        )?;
        conn.execute(
            "INSERT INTO memory_layers (memory_id, memory_kind, layer, status, created_at, updated_at)
             VALUES (?1, 'fact', ?2, 'active', ?3, ?3)",
            params![id, layer.as_str(), now],
        )?;
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
        source_episode_id: Option<&str>,
    ) -> Result<EdgeRecord> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let now = now_ts();
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
                source_episode_id,
                layer.as_str(),
                now
            ],
        )?;
        conn.execute(
            "INSERT INTO memory_layers (memory_id, memory_kind, layer, status, created_at, updated_at)
             VALUES (?1, 'edge', ?2, 'active', ?3, ?3)",
            params![id, layer.as_str(), now],
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

    pub fn get_entity(&self, id: &str) -> Result<Option<EntityRecord>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let row = conn
            .query_row(
                "SELECT id, entity_type, canonical_name, confidence, source_episode_id, layer,
                        created_at, updated_at, archived_at, invalidated_at, hit_count
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
                        row.get::<_, Option<i64>>(8)?,
                        row.get::<_, Option<i64>>(9)?,
                        row.get::<_, i64>(10)?,
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
            archived_at: row.8.map(ts_to_dt),
            invalidated_at: row.9.map(ts_to_dt),
            hit_count: row.10.max(0) as u64,
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
                   AND (e.normalized_name = ?1 OR a.normalized_alias = ?1)",
            )?;
            let entity_ids: Vec<String> = entity_stmt
                .query_map(params![normalized], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            drop(entity_stmt);

            let mut episode_stmt = conn.prepare(
                "SELECT id FROM episodes
                 WHERE archived_at IS NULL AND normalized_content = ?1",
            )?;
            let episode_ids: Vec<String> = episode_stmt
                .query_map(params![normalize_text(query)], |row| row.get(0))?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            (entity_ids, episode_ids)
        };

        let mut result = Vec::new();
        for entity_id in entity_ids {
            if let Some(record) = self.get_entity(&entity_id)? {
                result.push(MemoryRecord::Entity(record));
            }
        }
        for episode_id in episode_ids {
            if let Some(record) = self.get_episode(&episode_id)? {
                result.push(MemoryRecord::Episode(record));
            }
        }

        Ok(result)
    }

    pub fn related_graph_records(
        &self,
        entity_ids: &[String],
        hops: usize,
        limit: usize,
    ) -> Result<Vec<(MemoryRecord, usize)>> {
        if entity_ids.is_empty() {
            return Ok(Vec::new());
        }
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let mut all_records = Vec::new();
        let mut frontier: HashSet<String> = entity_ids.iter().cloned().collect();
        let mut visited: HashSet<String> = entity_ids.iter().cloned().collect();

        for hop in 1..=hops {
            if frontier.is_empty() || all_records.len() >= limit {
                break;
            }
            let current: Vec<String> = frontier.drain().collect();
            let placeholders = (1..=current.len())
                .map(|index| format!("?{}", index))
                .collect::<Vec<_>>()
                .join(",");
            let sql = format!(
                "SELECT id, subject_entity_id, predicate, object_entity_id, weight, source_episode_id,
                        layer, valid_from, valid_to, created_at, updated_at, archived_at, invalidated_at, hit_count
                 FROM edges
                 WHERE archived_at IS NULL
                   AND (subject_entity_id IN ({0}) OR object_entity_id IN ({0}))
                 LIMIT {1}",
                placeholders, limit
            );
            let mut edge_stmt = conn.prepare(&sql)?;
            let edges = edge_stmt
                .query_map(rusqlite::params_from_iter(current.iter()), map_edge)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for edge in edges {
                let subject = edge.subject_entity_id.clone();
                let object = edge.object_entity_id.clone();
                if visited.insert(subject.clone()) {
                    frontier.insert(subject);
                }
                if visited.insert(object.clone()) {
                    frontier.insert(object);
                }
                all_records.push((MemoryRecord::Edge(edge), hop));
                if all_records.len() >= limit {
                    break;
                }
            }
            if all_records.len() >= limit {
                break;
            }

            let sql = format!(
                "SELECT id, subject_entity_id, subject_text, predicate, object_entity_id, object_text,
                        layer, confidence, source_episode_id, valid_from, valid_to, created_at, updated_at, archived_at, invalidated_at, hit_count
                 FROM facts
                 WHERE archived_at IS NULL
                   AND ((subject_entity_id IS NOT NULL AND subject_entity_id IN ({0}))
                     OR (object_entity_id IS NOT NULL AND object_entity_id IN ({0})))
                 LIMIT {1}",
                placeholders, limit.saturating_sub(all_records.len())
            );
            let mut fact_stmt = conn.prepare(&sql)?;
            let facts = fact_stmt
                .query_map(rusqlite::params_from_iter(current.iter()), map_fact)?
                .collect::<rusqlite::Result<Vec<_>>>()?;
            for fact in facts {
                all_records.push((MemoryRecord::Fact(fact), hop));
                if all_records.len() >= limit {
                    break;
                }
            }
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

    pub fn load_l3_records(&self, limit: usize) -> Result<Vec<MemoryRecord>> {
        let rows = {
            let conn = self.conn.lock().expect("sqlite mutex poisoned");
            let mut stmt = conn.prepare(
                "SELECT memory_id, memory_kind FROM memory_layers
                 WHERE layer = 'L3' AND status = 'active'
                 ORDER BY updated_at DESC
                 LIMIT ?1",
            )?;
            let rows = stmt.query_map(params![limit as i64], |row| {
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

        Ok(records)
    }

    pub fn increment_hit_count(&self, memory: &MemoryRecord) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let sql = match memory {
            MemoryRecord::Episode(_) => {
                "UPDATE episodes SET hit_count = hit_count + 1, last_seen_at = ?2 WHERE id = ?1"
            }
            MemoryRecord::Entity(_) => {
                "UPDATE entities SET hit_count = hit_count + 1, last_seen_at = ?2 WHERE id = ?1"
            }
            MemoryRecord::Fact(_) => {
                "UPDATE facts SET hit_count = hit_count + 1, updated_at = ?2 WHERE id = ?1"
            }
            MemoryRecord::Edge(_) => {
                "UPDATE edges SET hit_count = hit_count + 1, updated_at = ?2 WHERE id = ?1"
            }
        };
        conn.execute(sql, params![memory.id(), now_ts()])?;
        Ok(())
    }

    pub fn update_layer(&self, kind: &str, id: &str, layer: MemoryLayer) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let table = table_for_kind(kind)?;
        let now = now_ts();
        let sql = format!(
            "UPDATE {} SET layer = ?2, updated_at = ?3 WHERE id = ?1",
            table
        );
        conn.execute(&sql, params![id, layer.as_str(), now])?;
        conn.execute(
            "UPDATE memory_layers
             SET layer = ?2, updated_at = ?3
             WHERE memory_id = ?1 AND memory_kind = ?4",
            params![id, layer.as_str(), now, kind],
        )?;
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
        Ok(())
    }

    pub fn record_index_state(
        &self,
        name: &str,
        doc_count: usize,
        status: &str,
        detail: Option<&str>,
    ) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.execute(
            "INSERT INTO index_state (index_name, doc_count, status, detail, last_rebuilt_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(index_name) DO UPDATE SET
                 doc_count = excluded.doc_count,
                 status = excluded.status,
                 detail = excluded.detail,
                 last_rebuilt_at = excluded.last_rebuilt_at",
            params![name, doc_count as i64, status, detail, now_ts()],
        )?;
        Ok(())
    }

    pub fn index_status(&self, name: &str) -> Result<IndexStatus> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let status = conn
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
                    })
                },
            )
            .optional()?;
        Ok(status.unwrap_or(IndexStatus {
            name: name.to_string(),
            doc_count: 0,
            status: "unknown".to_string(),
            detail: None,
        }))
    }

    pub fn create_consolidation_job(&self, trigger: &str) -> Result<()> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.execute(
            "INSERT INTO consolidation_jobs
             (id, trigger, status, created_at, updated_at)
             VALUES (?1, ?2, 'started', ?3, ?3)",
            params![Uuid::new_v4().to_string(), trigger, now_ts()],
        )?;
        Ok(())
    }

    pub fn complete_consolidation_jobs(&self) -> Result<usize> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let updated = conn.execute(
            "UPDATE consolidation_jobs SET status = 'completed', updated_at = ?1 WHERE status = 'started'",
            params![now_ts()],
        )?;
        Ok(updated)
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

    pub fn support_scope_key_for_episode(&self, episode_id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        conn.query_row(
            "SELECT COALESCE(session_id, id) FROM episodes WHERE id = ?1 LIMIT 1",
            params![episode_id],
            |row| row.get(0),
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

        CREATE TABLE IF NOT EXISTS consolidation_jobs (
            id TEXT PRIMARY KEY,
            trigger TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS index_state (
            index_name TEXT PRIMARY KEY,
            doc_count INTEGER NOT NULL,
            status TEXT NOT NULL,
            detail TEXT NULL,
            last_rebuilt_at INTEGER NULL
        );
        "#,
    )?;
    ensure_column(conn, "facts", "valid_from", "INTEGER NULL")?;
    ensure_column(conn, "facts", "valid_to", "INTEGER NULL")?;
    ensure_column(conn, "edges", "valid_from", "INTEGER NULL")?;
    ensure_column(conn, "edges", "valid_to", "INTEGER NULL")?;
    ensure_column(conn, "episodes", "session_id", "TEXT NULL")?;
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
    use super::Database;
    use anyhow::Result;
    use rusqlite::Connection;
    use tempfile::TempDir;

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
            CREATE TABLE consolidation_jobs (
                id TEXT PRIMARY KEY,
                trigger TEXT NOT NULL,
                status TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
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
}

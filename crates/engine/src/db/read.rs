use super::*;

impl Database {
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
}

use super::*;

impl Database {
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
    pub fn active_related_ids_for_episode(
        &self,
        kind: &str,
        episode_id: &str,
    ) -> Result<Vec<String>> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let table = table_for_kind(kind)?;
        let sql = format!(
            "SELECT id FROM {table}
             WHERE source_episode_id = ?1
               AND archived_at IS NULL
               AND invalidated_at IS NULL"
        );
        let mut stmt = conn.prepare(&sql)?;
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
    pub fn active_fact_count_for_episode(&self, episode_id: &str) -> Result<usize> {
        let conn = self.conn.lock().expect("sqlite mutex poisoned");
        let count = conn.query_row(
            "SELECT COUNT(*) FROM facts
             WHERE source_episode_id = ?1
               AND archived_at IS NULL
               AND invalidated_at IS NULL",
            params![episode_id],
            |row| row.get::<_, i64>(0),
        )?;
        Ok(count.max(0) as usize)
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

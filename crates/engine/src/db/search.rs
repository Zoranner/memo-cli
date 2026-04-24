use super::*;

impl Database {
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

pub(super) fn find_active_entity_id_by_reference(
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

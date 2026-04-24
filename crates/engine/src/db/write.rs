use super::{search::find_active_entity_id_by_reference, *};

impl Database {
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
}

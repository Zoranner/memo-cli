use super::{Database, ObservationContext};
use anyhow::Result;
use chrono::{TimeZone, Utc};
use rusqlite::{Connection, OptionalExtension};
use tempfile::TempDir;

use crate::types::{
    EntityInput, EpisodeInput, ExtractionSource, FactInput, MemoryLayer, MemoryRecord,
};

fn dt_to_ts(dt: chrono::DateTime<Utc>) -> i64 {
    dt.timestamp_millis()
}

#[test]
fn open_sets_current_schema_user_version() -> Result<()> {
    let temp = TempDir::new()?;
    let db_path = temp.path().join("memory.db");

    let _db = Database::open(&db_path)?;
    let conn = Connection::open(&db_path)?;
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;

    assert_eq!(user_version, 1);
    Ok(())
}

#[test]
fn open_rejects_database_newer_than_current_schema_version() -> Result<()> {
    let temp = TempDir::new()?;
    let db_path = temp.path().join("memory.db");

    let conn = Connection::open(&db_path)?;
    conn.execute_batch("PRAGMA user_version = 999;")?;
    drop(conn);

    let error = match Database::open(&db_path) {
        Ok(_) => anyhow::bail!("expected newer schema to be rejected"),
        Err(error) => error,
    };
    assert!(
        error
            .to_string()
            .contains("database schema version 999 is newer"),
        "unexpected error: {error:#}"
    );
    Ok(())
}

#[test]
fn rusqlite_storage_baseline_covers_spike_acceptance_flow() -> Result<()> {
    let temp = TempDir::new()?;
    let db = Database::open(&temp.path().join("memory.db"))?;
    let observed_at = Utc.with_ymd_and_hms(2026, 4, 26, 9, 0, 0).unwrap();

    let episode = db.insert_episode(
        &EpisodeInput {
            content: "Alice moved from Paris to London.".to_string(),
            layer: MemoryLayer::L1,
            entities: Vec::new(),
            facts: Vec::new(),
            source_episode_id: None,
            session_id: Some("storage-spike-session".to_string()),
            recorded_at: Some(observed_at),
            confidence: 0.91,
        },
        Some(&[0.1, 0.2, 0.3, 0.4]),
    )?;
    let loaded_episode = db
        .get_episode(&episode.id)?
        .expect("expected inserted episode to load");
    assert_eq!(loaded_episode.id, episode.id);
    assert_eq!(loaded_episode.content, episode.content);
    assert_eq!(loaded_episode.session_id, episode.session_id);

    let alice = db.upsert_entity(
        &EntityInput {
            entity_type: "person".to_string(),
            name: "Alice".to_string(),
            aliases: vec!["Ally".to_string()],
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
            confidence: 0.93,
            source: ExtractionSource::Manual,
        },
        MemoryLayer::L1,
        ObservationContext {
            source_episode_id: Some(&episode.id),
            observed_at,
        },
        None,
    )?;
    let london = db.upsert_entity(
        &EntityInput {
            entity_type: "place".to_string(),
            name: "London".to_string(),
            aliases: Vec::new(),
            confidence: 0.96,
            source: ExtractionSource::Manual,
        },
        MemoryLayer::L1,
        ObservationContext {
            source_episode_id: Some(&episode.id),
            observed_at,
        },
        None,
    )?;

    let alias_hit = db
        .resolve_active_entity_reference("ally")?
        .expect("expected alias to resolve to Alice");
    assert_eq!(alias_hit.id, alice.id);
    let exact_alias_hits = db.search_exact_alias("Ally")?;
    assert!(exact_alias_hits
        .iter()
        .any(|record| matches!(record, MemoryRecord::Entity(entity) if entity.id == alice.id)));

    db.ensure_mention(&episode.id, &alice.id, "subject", 0.95)?;
    db.ensure_mention(&episode.id, &paris.id, "old_location", 0.93)?;
    db.ensure_mention(&episode.id, &london.id, "new_location", 0.96)?;

    let paris_fact = db.insert_fact(
        &FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "Paris".to_string(),
            confidence: 0.72,
            source: ExtractionSource::Manual,
        },
        MemoryLayer::L1,
        Some(&alice.id),
        Some(&paris.id),
        ObservationContext {
            source_episode_id: Some(&episode.id),
            observed_at,
        },
        None,
    )?;
    let london_fact = db.insert_fact(
        &FactInput {
            subject: "Alice".to_string(),
            predicate: "lives_in".to_string(),
            object: "London".to_string(),
            confidence: 0.96,
            source: ExtractionSource::Manual,
        },
        MemoryLayer::L1,
        Some(&alice.id),
        Some(&london.id),
        ObservationContext {
            source_episode_id: Some(&episode.id),
            observed_at,
        },
        None,
    )?;
    let london_edge = db.insert_edge(
        &alice.id,
        "lives_in",
        &london.id,
        0.96,
        MemoryLayer::L1,
        ObservationContext {
            source_episode_id: Some(&episode.id),
            observed_at,
        },
    )?;

    let conflicting_objects = db
        .active_facts_in_layers(&[MemoryLayer::L1])?
        .into_iter()
        .filter(|fact| fact.subject_text == "Alice" && fact.predicate == "lives_in")
        .map(|fact| fact.object_text)
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(
        conflicting_objects,
        ["London".to_string(), "Paris".to_string()]
            .into_iter()
            .collect()
    );

    db.update_layer("episode", &episode.id, MemoryLayer::L2)?;
    db.update_layer("entity", &alice.id, MemoryLayer::L2)?;
    db.update_layer("fact", &london_fact.id, MemoryLayer::L2)?;
    db.update_layer("edge", &london_edge.id, MemoryLayer::L2)?;
    assert!(matches!(
        db.get_memory_by_kind("fact", &london_fact.id)?,
        Some(MemoryRecord::Fact(fact)) if fact.layer == MemoryLayer::L2
    ));

    let text_jobs = db.load_outstanding_index_jobs("text")?;
    assert!(text_jobs.iter().any(|job| job.memory_id == episode.id));
    assert!(text_jobs.iter().any(|job| job.memory_id == alice.id));
    assert!(text_jobs.iter().any(|job| job.memory_id == paris_fact.id));
    assert!(text_jobs.iter().any(|job| job.memory_id == london_fact.id));
    assert!(text_jobs.iter().all(|job| !job.failed));

    let vector_jobs = db.load_outstanding_index_jobs("vector")?;
    assert_eq!(vector_jobs.len(), 1);
    assert_eq!(vector_jobs[0].memory_id, episode.id);
    assert!(!vector_jobs[0].failed);

    let text_job_ids = text_jobs
        .iter()
        .map(|job| job.id.clone())
        .collect::<Vec<_>>();
    db.clear_index_jobs("text", &text_job_ids)?;
    assert!(db.load_outstanding_index_jobs("text")?.is_empty());

    let vector_job_ids = vector_jobs
        .iter()
        .map(|job| job.id.clone())
        .collect::<Vec<_>>();
    db.clear_index_jobs("vector", &vector_job_ids)?;
    assert!(db.load_outstanding_index_jobs("vector")?.is_empty());

    Ok(())
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
    let user_version: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(user_version, 1);

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

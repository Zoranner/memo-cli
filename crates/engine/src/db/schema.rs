use anyhow::Result;
use rusqlite::Connection;

pub(super) fn init_schema(conn: &Connection) -> Result<()> {
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

use anyhow::Result;
use rusqlite::{params, Connection, OptionalExtension};
use uuid::Uuid;

use super::{now_ts, IndexJobOperation};

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

pub(super) fn queue_text_index_job(
    conn: &Connection,
    memory_kind: &str,
    memory_id: &str,
    operation: IndexJobOperation,
) -> Result<()> {
    queue_index_job(conn, "text", memory_kind, memory_id, operation)
}

pub(super) fn queue_vector_index_job(
    conn: &Connection,
    memory_kind: &str,
    memory_id: &str,
    operation: IndexJobOperation,
) -> Result<()> {
    queue_index_job(conn, "vector", memory_kind, memory_id, operation)
}

pub(super) fn queue_index_delete_jobs(conn: &Connection, kind: &str, id: &str) -> Result<()> {
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

pub(super) fn clear_index_jobs_by_ids(
    conn: &Connection,
    index_name: &str,
    job_ids: &[String],
) -> Result<()> {
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

pub(super) fn fail_index_jobs_by_ids(
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

pub(super) fn index_job_observability(
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

pub(super) fn record_index_ready(
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

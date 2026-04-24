use super::*;

impl Database {
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
                failed: row.get::<_, String>(4)? == "failed",
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
}

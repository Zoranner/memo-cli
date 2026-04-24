use super::*;

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
}

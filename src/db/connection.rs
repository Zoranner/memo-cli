use anyhow::{Context, Result};
use lancedb::connection::Connection as LanceConnection;

pub struct Connection {
    pub conn: LanceConnection,
}

impl Connection {
    pub async fn connect(path: &str) -> Result<Self> {
        let conn = lancedb::connect(path)
            .execute()
            .await
            .with_context(|| format!("Failed to connect to database: {}", path))?;

        Ok(Self { conn })
    }

    pub fn inner(&self) -> &LanceConnection {
        &self.conn
    }
}

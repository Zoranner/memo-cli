use anyhow::{Context, Result};
use lancedb::connection::Connection as LanceConnection;
use std::path::Path;

pub struct Connection {
    pub conn: LanceConnection,
}

impl Connection {
    /// 连接到 LanceDB 数据库
    ///
    /// # Arguments
    /// * `path` - 数据库路径
    pub async fn connect(path: &Path) -> Result<Self> {
        let path_str = path
            .to_str()
            .with_context(|| format!("Invalid UTF-8 in path: {}", path.display()))?;

        let conn = lancedb::connect(path_str)
            .execute()
            .await
            .with_context(|| format!("Failed to connect to database: {}", path.display()))?;

        Ok(Self { conn })
    }

    pub fn inner(&self) -> &LanceConnection {
        &self.conn
    }
}

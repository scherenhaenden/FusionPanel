#![forbid(unsafe_code)]

use rusqlite::{Connection, OptionalExtension, Result};
use std::sync::{Arc, Mutex};

pub trait ContainerStore: Send + Sync {
    fn create_container(&self, id: &str, tenant_id: &str, name: &str) -> Result<()>;
    fn get_container_tenant(&self, id: &str) -> Result<Option<String>>;
    fn remove_container(&self, id: &str) -> Result<()>;
    fn get_all_containers(&self) -> Result<Vec<(String, String)>>;
}

pub struct SqliteContainerStore {
    conn: Arc<Mutex<Connection>>,
}

impl SqliteContainerStore {
    pub fn new(conn: Connection) -> Result<Self> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS containers (
                id TEXT PRIMARY KEY,
                tenant_id TEXT NOT NULL,
                name TEXT NOT NULL,
                created_at DATETIME DEFAULT CURRENT_TIMESTAMP
            )",
            [],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl ContainerStore for SqliteContainerStore {
    fn create_container(&self, id: &str, tenant_id: &str, name: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO containers (id, tenant_id, name) VALUES (?1, ?2, ?3)",
            [id, tenant_id, name],
        )?;
        Ok(())
    }

    fn get_container_tenant(&self, id: &str) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT tenant_id FROM containers WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .optional()
    }

    fn remove_container(&self, id: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM containers WHERE id = ?1", [id])?;
        Ok(())
    }

    fn get_all_containers(&self) -> Result<Vec<(String, String)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT id, tenant_id FROM containers")?;
        let rows = stmt.query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        let mut containers = Vec::new();
        for row in rows {
            containers.push(row?);
        }
        Ok(containers)
    }
}

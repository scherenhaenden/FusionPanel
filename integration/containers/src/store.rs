#![forbid(unsafe_code)]

use rusqlite::{Connection, OptionalExtension, Result};
use std::sync::{Arc, Mutex};

pub trait ContainerStore: Send + Sync {
    fn create_container(&self, id: &str, tenant_id: &str, name: &str, ports: &[u16]) -> Result<()>;
    fn get_container_tenant(&self, id: &str) -> Result<Option<String>>;
    fn get_container_ports(&self, id: &str) -> Result<Vec<u16>>;
    fn get_all_container_ports(&self) -> Result<Vec<u16>>;
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
        conn.execute(
            "CREATE TABLE IF NOT EXISTS container_ports (
                container_id TEXT NOT NULL,
                host_port INTEGER NOT NULL UNIQUE,
                PRIMARY KEY (container_id, host_port),
                FOREIGN KEY (container_id) REFERENCES containers(id) ON DELETE CASCADE
            )",
            [],
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }
}

impl ContainerStore for SqliteContainerStore {
    fn create_container(&self, id: &str, tenant_id: &str, name: &str, ports: &[u16]) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT INTO containers (id, tenant_id, name) VALUES (?1, ?2, ?3)",
            [id, tenant_id, name],
        )?;
        for port in ports {
            tx.execute(
                "INSERT INTO container_ports (container_id, host_port) VALUES (?1, ?2)",
                rusqlite::params![id, port],
            )?;
        }
        tx.commit()
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

    fn get_container_ports(&self, id: &str) -> Result<Vec<u16>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT host_port FROM container_ports WHERE container_id = ?1 ORDER BY host_port",
        )?;
        let rows = stmt.query_map([id], |row| row.get(0))?;
        rows.collect()
    }

    fn get_all_container_ports(&self) -> Result<Vec<u16>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare("SELECT host_port FROM container_ports ORDER BY host_port")?;
        let rows = stmt.query_map([], |row| row.get(0))?;
        rows.collect()
    }

    fn remove_container(&self, id: &str) -> Result<()> {
        let mut conn = self.conn.lock().unwrap();
        let tx = conn.transaction()?;
        tx.execute("DELETE FROM container_ports WHERE container_id = ?1", [id])?;
        tx.execute("DELETE FROM containers WHERE id = ?1", [id])?;
        tx.commit()
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

use crate::incidents::{Incident, IncidentState};
use rusqlite::{params, Connection, Result as SqlResult};

pub struct IncidentStore {
    conn: Connection,
}

impl IncidentStore {
    pub fn new(path: &std::path::Path) -> SqlResult<Self> {
        let conn = Connection::open(path)?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS incidents (
                id TEXT PRIMARY KEY,
                tenant_id TEXT,
                title TEXT NOT NULL,
                state TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn in_memory() -> SqlResult<Self> {
        let conn = Connection::open_in_memory()?;

        conn.execute(
            "CREATE TABLE IF NOT EXISTS incidents (
                id TEXT PRIMARY KEY,
                tenant_id TEXT,
                title TEXT NOT NULL,
                state TEXT NOT NULL,
                created_at INTEGER NOT NULL,
                updated_at INTEGER NOT NULL
            )",
            [],
        )?;

        Ok(Self { conn })
    }

    pub fn save_incident(&self, incident: &Incident) -> SqlResult<()> {
        let state_str = match incident.state {
            IncidentState::Triggered => "Triggered",
            IncidentState::Acknowledged => "Acknowledged",
            IncidentState::Resolved => "Resolved",
        };

        self.conn.execute(
            "INSERT INTO incidents (id, tenant_id, title, state, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET
             title=excluded.title,
             state=excluded.state,
             updated_at=excluded.updated_at",
            params![
                incident.id,
                incident.tenant_id,
                incident.title,
                state_str,
                incident.created_at,
                incident.updated_at,
            ],
        )?;

        Ok(())
    }

    pub fn get_incident(&self, id: &str) -> SqlResult<Option<Incident>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, tenant_id, title, state, created_at, updated_at FROM incidents WHERE id = ?1",
        )?;

        let mut iter = stmt.query_map(params![id], |row| {
            let state_str: String = row.get(3)?;
            let state = match state_str.as_str() {
                "Triggered" => IncidentState::Triggered,
                "Acknowledged" => IncidentState::Acknowledged,
                "Resolved" => IncidentState::Resolved,
                _ => IncidentState::Triggered, // Fallback
            };

            Ok(Incident {
                id: row.get(0)?,
                tenant_id: row.get(1)?,
                title: row.get(2)?,
                state,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
            })
        })?;

        if let Some(res) = iter.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn get_active_incidents(&self, tenant_id: Option<&str>) -> SqlResult<Vec<Incident>> {
        let mut incidents = Vec::new();
        if let Some(tid) = tenant_id {
            let mut stmt = self.conn.prepare("SELECT id, tenant_id, title, state, created_at, updated_at FROM incidents WHERE state != 'Resolved' AND (tenant_id = ?1 OR tenant_id IS NULL)")?;

            let iter = stmt.query_map(params![tid], |row| {
                let state_str: String = row.get(3)?;
                let state = match state_str.as_str() {
                    "Triggered" => IncidentState::Triggered,
                    "Acknowledged" => IncidentState::Acknowledged,
                    _ => IncidentState::Triggered, // Fallback
                };

                Ok(Incident {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    title: row.get(2)?,
                    state,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?;
            for item in iter {
                incidents.push(item?);
            }
        } else {
            let mut stmt = self.conn.prepare("SELECT id, tenant_id, title, state, created_at, updated_at FROM incidents WHERE state != 'Resolved'")?;

            let iter = stmt.query_map([], |row| {
                let state_str: String = row.get(3)?;
                let state = match state_str.as_str() {
                    "Triggered" => IncidentState::Triggered,
                    "Acknowledged" => IncidentState::Acknowledged,
                    _ => IncidentState::Triggered, // Fallback
                };

                Ok(Incident {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    title: row.get(2)?,
                    state,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?;
            for item in iter {
                incidents.push(item?);
            }
        }

        Ok(incidents)
    }

    pub fn get_all_incidents(&self, tenant_id: Option<&str>) -> SqlResult<Vec<Incident>> {
        let mut incidents = Vec::new();

        if let Some(tid) = tenant_id {
            let mut stmt = self.conn.prepare("SELECT id, tenant_id, title, state, created_at, updated_at FROM incidents WHERE tenant_id = ?1 OR tenant_id IS NULL ORDER BY created_at DESC")?;

            let iter = stmt.query_map(params![tid], |row| {
                let state_str: String = row.get(3)?;
                let state = match state_str.as_str() {
                    "Triggered" => IncidentState::Triggered,
                    "Acknowledged" => IncidentState::Acknowledged,
                    "Resolved" => IncidentState::Resolved,
                    _ => IncidentState::Triggered, // Fallback
                };

                Ok(Incident {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    title: row.get(2)?,
                    state,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?;
            for item in iter {
                incidents.push(item?);
            }
        } else {
            let mut stmt = self.conn.prepare("SELECT id, tenant_id, title, state, created_at, updated_at FROM incidents ORDER BY created_at DESC")?;

            let iter = stmt.query_map([], |row| {
                let state_str: String = row.get(3)?;
                let state = match state_str.as_str() {
                    "Triggered" => IncidentState::Triggered,
                    "Acknowledged" => IncidentState::Acknowledged,
                    "Resolved" => IncidentState::Resolved,
                    _ => IncidentState::Triggered, // Fallback
                };

                Ok(Incident {
                    id: row.get(0)?,
                    tenant_id: row.get(1)?,
                    title: row.get(2)?,
                    state,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?;

            for item in iter {
                incidents.push(item?);
            }
        }

        Ok(incidents)
    }
}

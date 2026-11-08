use chrono::Utc;
use nodes::state::NodeState;
use rusqlite::{params, Connection, OptionalExtension};

use uuid::Uuid;

#[derive(thiserror::Error, Debug)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Serialization error: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Node not found")]
    NotFound,
    #[error("Invalid state transition")]
    InvalidTransition,
}

pub struct NodeStore {
    conn: Connection,
}

impl NodeStore {
    pub fn new(conn: Connection) -> Result<Self, StoreError> {
        conn.execute(
            "CREATE TABLE IF NOT EXISTS nodes (
                id TEXT PRIMARY KEY,
                state TEXT NOT NULL,
                last_updated TEXT NOT NULL
            )",
            [],
        )?;
        Ok(Self { conn })
    }

    pub fn insert_node(&self, id: Uuid, state: &NodeState) -> Result<(), StoreError> {
        let state_json = serde_json::to_string(state)?;
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO nodes (id, state, last_updated) VALUES (?1, ?2, ?3)",
            params![id.to_string(), state_json, now],
        )?;
        Ok(())
    }

    pub fn get_node(&self, id: Uuid) -> Result<NodeState, StoreError> {
        let state_json: String = self
            .conn
            .query_row(
                "SELECT state FROM nodes WHERE id = ?1",
                params![id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound)?;
        let state: NodeState = serde_json::from_str(&state_json)?;
        Ok(state)
    }

    pub fn update_node_state(&self, id: Uuid, state: &NodeState) -> Result<(), StoreError> {
        let state_json = serde_json::to_string(state)?;
        let now = Utc::now().to_rfc3339();
        let rows = self.conn.execute(
            "UPDATE nodes SET state = ?1, last_updated = ?2 WHERE id = ?3",
            params![state_json, now, id.to_string()],
        )?;
        if rows == 0 {
            return Err(StoreError::NotFound);
        }
        Ok(())
    }
}

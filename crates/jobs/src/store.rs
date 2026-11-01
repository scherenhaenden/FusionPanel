#![forbid(unsafe_code)]

use std::path::Path;

use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

use crate::models::{Job, JobId, JobState, JobStoreError};

pub struct JobStore {
    conn: Connection,
}

impl JobStore {
    /// Create a new persistent job store at the given path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, JobStoreError> {
        let conn = Connection::open(path)?;
        Self::init(&conn)?;
        Ok(Self { conn })
    }

    /// Create a new in-memory job store (useful for testing).
    pub fn in_memory() -> Result<Self, JobStoreError> {
        let conn = Connection::open_in_memory()?;
        Self::init(&conn)?;
        Ok(Self { conn })
    }

    fn init(conn: &Connection) -> Result<(), JobStoreError> {
        conn.execute(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                payload TEXT NOT NULL,
                state TEXT NOT NULL,
                worker_id TEXT,
                error_text TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
            [],
        )?;
        Ok(())
    }

    /// Create a new job.
    pub fn create(&self, kind: &str, payload: &Value) -> Result<JobId, JobStoreError> {
        let id = Uuid::new_v4().to_string();
        let payload_str = serde_json::to_string(payload)
            .map_err(|e| JobStoreError::MalformedData(e.to_string()))?;
        let now = Utc::now().to_rfc3339();

        self.conn.execute(
            "INSERT INTO jobs (id, kind, payload, state, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, kind, payload_str, JobState::Queued.as_str(), now, now],
        )?;

        Ok(id)
    }

    /// Get a job by ID.
    pub fn get(&self, id: &JobId) -> Result<Job, JobStoreError> {
        let mut stmt = self.conn.prepare("SELECT id, kind, payload, state, worker_id, error_text, created_at, updated_at FROM jobs WHERE id = ?1")?;
        let job = stmt.query_row([id], Self::row_to_job).optional()?;
        job.ok_or_else(|| JobStoreError::NotFound(id.clone()))
    }

    /// List all jobs.
    pub fn list(&self) -> Result<Vec<Job>, JobStoreError> {
        let mut stmt = self.conn.prepare("SELECT id, kind, payload, state, worker_id, error_text, created_at, updated_at FROM jobs")?;
        let rows = stmt.query_map([], Self::row_to_job)?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    /// List all running jobs (useful for explicit reconciliation).
    pub fn list_running(&self) -> Result<Vec<Job>, JobStoreError> {
        let mut stmt = self.conn.prepare("SELECT id, kind, payload, state, worker_id, error_text, created_at, updated_at FROM jobs WHERE state = ?1")?;
        let rows = stmt.query_map([JobState::Running.as_str()], Self::row_to_job)?;

        let mut jobs = Vec::new();
        for row in rows {
            jobs.push(row?);
        }
        Ok(jobs)
    }

    /// Atomically claim the oldest queued job for a worker.
    pub fn claim(&mut self, worker_id: &str) -> Result<Option<Job>, JobStoreError> {
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let job_id: Option<String> = {
            let mut stmt =
                tx.prepare("SELECT id FROM jobs WHERE state = ?1 ORDER BY created_at ASC LIMIT 1")?;
            stmt.query_row([JobState::Queued.as_str()], |row| row.get(0))
                .optional()?
        };

        if let Some(id) = job_id {
            let now = Utc::now().to_rfc3339();
            tx.execute(
                "UPDATE jobs SET state = ?1, worker_id = ?2, updated_at = ?3 WHERE id = ?4",
                params![JobState::Running.as_str(), worker_id, now, id],
            )?;
            tx.commit()?;
            Ok(Some(self.get(&id)?))
        } else {
            Ok(None)
        }
    }

    /// Transition a job to a new state.
    pub fn transition(
        &mut self,
        id: &JobId,
        new_state: JobState,
        error_text: Option<String>,
    ) -> Result<(), JobStoreError> {
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let current_state_str: String = tx
            .query_row("SELECT state FROM jobs WHERE id = ?1", [id], |row| {
                row.get(0)
            })
            .optional()?
            .ok_or_else(|| JobStoreError::NotFound(id.clone()))?;

        let current_state = current_state_str.parse::<JobState>().map_err(|_| {
            JobStoreError::MalformedData(format!("Unknown state: {}", current_state_str))
        })?;

        // Enforce valid state transitions
        let is_valid = matches!(
            (current_state, new_state),
            (JobState::Queued, JobState::Running)
                | (JobState::Queued, JobState::Cancelled)
                | (JobState::Running, JobState::Succeeded)
                | (JobState::Running, JobState::Failed)
                | (JobState::Running, JobState::Cancelled)
        );

        if !is_valid {
            return Err(JobStoreError::InvalidTransition {
                from: current_state,
                to: new_state,
            });
        }

        let now = Utc::now().to_rfc3339();
        tx.execute(
            "UPDATE jobs SET state = ?1, error_text = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_state.as_str(), error_text, now, id],
        )?;

        tx.commit()?;
        Ok(())
    }

    fn row_to_job(row: &rusqlite::Row) -> rusqlite::Result<Job> {
        let id: String = row.get(0)?;
        let kind: String = row.get(1)?;
        let payload_str: String = row.get(2)?;
        let state_str: String = row.get(3)?;
        let worker_id: Option<String> = row.get(4)?;
        let error_text: Option<String> = row.get(5)?;
        let created_at_str: String = row.get(6)?;
        let updated_at_str: String = row.get(7)?;

        let payload = serde_json::from_str(&payload_str).map_err(|_| {
            rusqlite::Error::InvalidColumnType(
                2,
                "payload".to_string(),
                rusqlite::types::Type::Text,
            )
        })?;

        let state = state_str.parse::<JobState>().map_err(|_| {
            rusqlite::Error::InvalidColumnType(3, "state".to_string(), rusqlite::types::Type::Text)
        })?;

        let created_at = chrono::DateTime::parse_from_rfc3339(&created_at_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    6,
                    "created_at".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);

        let updated_at = chrono::DateTime::parse_from_rfc3339(&updated_at_str)
            .map_err(|_| {
                rusqlite::Error::InvalidColumnType(
                    7,
                    "updated_at".to_string(),
                    rusqlite::types::Type::Text,
                )
            })?
            .with_timezone(&Utc);

        Ok(Job {
            id,
            kind,
            payload,
            state,
            worker_id,
            error_text,
            created_at,
            updated_at,
        })
    }
}

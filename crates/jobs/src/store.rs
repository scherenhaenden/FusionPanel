#![forbid(unsafe_code)]

use std::path::Path;

use chrono::{DateTime, Duration, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value;
use uuid::Uuid;

use crate::models::{Job, JobId, JobLogEntry, JobState, JobStoreError, MAX_ERROR_TEXT_BYTES};

const MAX_LOG_TEXT_BYTES: usize = 4096;
const MAX_JOB_LOG_BYTES: usize = 1024 * 1024;
pub const JOB_LEASE_TIMEOUT_SECONDS: i64 = 300;

pub struct JobStore {
    conn: Connection,
}

impl JobStore {
    /// Create a new persistent job store at the given path.
    pub fn new(path: impl AsRef<Path>) -> Result<Self, JobStoreError> {
        let conn = Connection::open(path)?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
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
        conn.execute_batch("PRAGMA foreign_keys = ON;")?;
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            r#"
            CREATE TABLE IF NOT EXISTS jobs (
                id TEXT PRIMARY KEY,
                kind TEXT NOT NULL,
                payload TEXT NOT NULL,
                state TEXT NOT NULL,
                worker_id TEXT,
                error_text TEXT,
                idempotency_key TEXT UNIQUE,
                idempotency_fingerprint TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
            );
            "#,
            [],
        )?;
        let has_idempotency_key = {
            let mut statement = tx.prepare("PRAGMA table_info(jobs)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            let has_column = columns
                .filter_map(Result::ok)
                .any(|column| column == "idempotency_key");
            has_column
        };
        if !has_idempotency_key {
            tx.execute("ALTER TABLE jobs ADD COLUMN idempotency_key TEXT", [])?;
        }
        tx.execute(
            "CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_idempotency_key ON jobs (idempotency_key) WHERE idempotency_key IS NOT NULL",
            [],
        )?;
        let has_idempotency_fingerprint = {
            let mut statement = tx.prepare("PRAGMA table_info(jobs)")?;
            let columns = statement.query_map([], |row| row.get::<_, String>(1))?;
            let has_column = columns
                .filter_map(Result::ok)
                .any(|column| column == "idempotency_fingerprint");
            has_column
        };
        if !has_idempotency_fingerprint {
            tx.execute(
                "ALTER TABLE jobs ADD COLUMN idempotency_fingerprint TEXT",
                [],
            )?;
        }
        tx.execute(
            "CREATE TABLE IF NOT EXISTS job_logs (job_id TEXT NOT NULL, sequence INTEGER NOT NULL, stream TEXT NOT NULL, text TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY (job_id, sequence), FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE)",
            [],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Create a new job.
    pub fn create(&self, kind: &str, payload: &Value) -> Result<JobId, JobStoreError> {
        let id = Uuid::new_v4().to_string();
        let payload_str = serde_json::to_string(payload)
            .map_err(|error| JobStoreError::MalformedData(error.to_string()))?;
        let now = Utc::now().to_rfc3339();
        self.conn.execute(
            "INSERT INTO jobs (id, kind, payload, state, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, kind, payload_str, JobState::Queued.as_str(), now],
        )?;
        Ok(id)
    }

    pub fn create_idempotent(
        &mut self,
        kind: &str,
        payload: &Value,
        idempotency_key: &str,
        fingerprint: &Value,
    ) -> Result<(JobId, bool), JobStoreError> {
        let key = idempotency_key.trim();
        if key.is_empty() || key.len() > 255 {
            return Err(JobStoreError::IdempotencyConflict);
        }

        let payload_str = serde_json::to_string(payload)
            .map_err(|error| JobStoreError::MalformedData(error.to_string()))?;
        let fingerprint_str = serde_json::to_string(fingerprint)
            .map_err(|error| JobStoreError::MalformedData(error.to_string()))?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some((existing_id, existing_kind, existing_fingerprint)) = tx
            .query_row(
                "SELECT id, kind, idempotency_fingerprint FROM jobs WHERE idempotency_key = ?1",
                [key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?
        {
            if existing_kind == kind
                && existing_fingerprint.as_deref() == Some(fingerprint_str.as_str())
            {
                tx.commit()?;
                return Ok((existing_id, false));
            }
            return Err(JobStoreError::IdempotencyConflict);
        }

        let id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();
        tx.execute(
            "INSERT INTO jobs (id, kind, payload, state, idempotency_key, idempotency_fingerprint, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![id, kind, payload_str, JobState::Queued.as_str(), key, fingerprint_str, now],
        )?;
        tx.commit()?;
        Ok((id, true))
    }

    /// Get a job by ID.
    pub fn get(&self, id: &JobId) -> Result<Job, JobStoreError> {
        let mut stmt = self.conn.prepare("SELECT id, kind, payload, state, worker_id, error_text, created_at, updated_at FROM jobs WHERE id = ?1")?;
        let job = stmt.query_row([id], Self::row_to_job).optional()?;
        job.ok_or_else(|| JobStoreError::NotFound(id.clone()))
    }

    /// List all jobs.
    pub fn list(&self) -> Result<Vec<Job>, JobStoreError> {
        let mut stmt = self.conn.prepare("SELECT id, kind, payload, state, worker_id, error_text, created_at, updated_at FROM jobs ORDER BY created_at, id")?;
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
        self.claim_at(worker_id, Utc::now())
    }

    fn claim_at(
        &mut self,
        worker_id: &str,
        now: DateTime<Utc>,
    ) -> Result<Option<Job>, JobStoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let lease_cutoff = (now - Duration::seconds(JOB_LEASE_TIMEOUT_SECONDS)).to_rfc3339();

        let job_id: Option<String> = {
            let mut stmt = tx.prepare(
                "SELECT id FROM jobs WHERE state = ?1 OR (state = ?2 AND julianday(updated_at) <= julianday(?3)) ORDER BY CASE WHEN state = ?1 THEN 0 ELSE 1 END, created_at ASC, id ASC LIMIT 1",
            )?;
            stmt.query_row(
                params![
                    JobState::Queued.as_str(),
                    JobState::Running.as_str(),
                    lease_cutoff
                ],
                |row| row.get(0),
            )
            .optional()?
        };

        if let Some(id) = job_id {
            let now = now.to_rfc3339();
            tx.execute(
                "UPDATE jobs SET state = ?1, worker_id = ?2, updated_at = ?3 WHERE id = ?4",
                params![JobState::Running.as_str(), worker_id, now, id],
            )?;
            // Decode before commit so a malformed row cannot leave a failed
            // claim persisted as Running. Return the transaction's snapshot.
            let job = tx.query_row(
                "SELECT id, kind, payload, state, worker_id, error_text, created_at, updated_at FROM jobs WHERE id = ?1",
                [&id],
                Self::row_to_job,
            )?;
            tx.commit()?;
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    /// Transition a job to a new state.
    ///
    /// Error text is limited to [`MAX_ERROR_TEXT_BYTES`] UTF-8 bytes. Oversized
    /// text is rejected without truncation or any mutation of the job.
    pub fn transition(
        &mut self,
        id: &JobId,
        new_state: JobState,
        error_text: Option<String>,
    ) -> Result<(), JobStoreError> {
        if let Some(text) = &error_text {
            if text.len() > MAX_ERROR_TEXT_BYTES {
                return Err(JobStoreError::ErrorTextTooLong {
                    max_bytes: MAX_ERROR_TEXT_BYTES,
                    actual_bytes: text.len(),
                });
            }
        }

        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

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

    pub fn transition_for_worker(
        &mut self,
        id: &JobId,
        worker_id: &str,
        new_state: JobState,
        error_text: Option<String>,
    ) -> Result<(), JobStoreError> {
        if let Some(text) = &error_text {
            if text.len() > MAX_ERROR_TEXT_BYTES {
                return Err(JobStoreError::ErrorTextTooLong {
                    max_bytes: MAX_ERROR_TEXT_BYTES,
                    actual_bytes: text.len(),
                });
            }
        }
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let record = tx
            .query_row(
                "SELECT state, worker_id FROM jobs WHERE id = ?1",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?
            .ok_or_else(|| JobStoreError::NotFound(id.clone()))?;
        if record.1.as_deref() != Some(worker_id) {
            return Err(JobStoreError::WrongWorker);
        }
        let current_state = record
            .0
            .parse::<JobState>()
            .map_err(|_| JobStoreError::MalformedData(format!("Unknown state: {}", record.0)))?;
        if !matches!(
            (current_state, new_state),
            (JobState::Running, JobState::Succeeded)
                | (JobState::Running, JobState::Failed)
                | (JobState::Running, JobState::Cancelled)
        ) {
            return Err(JobStoreError::InvalidTransition {
                from: current_state,
                to: new_state,
            });
        }
        tx.execute(
            "UPDATE jobs SET state = ?1, error_text = ?2, updated_at = ?3 WHERE id = ?4",
            params![new_state.as_str(), error_text, Utc::now().to_rfc3339(), id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn append_log(
        &mut self,
        id: &JobId,
        worker_id: &str,
        stream: &str,
        text: &str,
    ) -> Result<(), JobStoreError> {
        if !matches!(stream, "stdout" | "stderr" | "system") {
            return Err(JobStoreError::InvalidLogStream);
        }
        if text.len() > MAX_LOG_TEXT_BYTES {
            return Err(JobStoreError::LogTextTooLong {
                max_bytes: MAX_LOG_TEXT_BYTES,
                actual_bytes: text.len(),
            });
        }

        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let record = tx
            .query_row(
                "SELECT state, worker_id FROM jobs WHERE id = ?1",
                [id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?
            .ok_or_else(|| JobStoreError::NotFound(id.clone()))?;
        if record.0 != JobState::Running.as_str() || record.1.as_deref() != Some(worker_id) {
            return Err(JobStoreError::WrongWorker);
        }
        let existing_bytes: i64 = tx.query_row(
            "SELECT COALESCE(SUM(length(CAST(text AS BLOB))), 0) FROM job_logs WHERE job_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        if existing_bytes < 0 || existing_bytes as usize + text.len() > MAX_JOB_LOG_BYTES {
            return Err(JobStoreError::LogLimitExceeded {
                max_bytes: MAX_JOB_LOG_BYTES,
            });
        }
        let sequence: i64 = tx.query_row(
            "SELECT COALESCE(MAX(sequence), 0) + 1 FROM job_logs WHERE job_id = ?1",
            [id],
            |row| row.get(0),
        )?;
        tx.execute(
            "INSERT INTO job_logs (job_id, sequence, stream, text, created_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![id, sequence, stream, text, Utc::now().to_rfc3339()],
        )?;
        tx.execute(
            "UPDATE jobs SET updated_at = ?1 WHERE id = ?2",
            params![Utc::now().to_rfc3339(), id],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn list_logs(&self, id: &JobId) -> Result<Vec<JobLogEntry>, JobStoreError> {
        if self
            .conn
            .query_row("SELECT 1 FROM jobs WHERE id = ?1", [id], |_| Ok(()))
            .optional()?
            .is_none()
        {
            return Err(JobStoreError::NotFound(id.clone()));
        }
        let mut statement = self.conn.prepare(
            "SELECT sequence, stream, text, created_at FROM job_logs WHERE job_id = ?1 ORDER BY sequence",
        )?;
        let rows = statement.query_map([id], |row| {
            let timestamp: String = row.get(3)?;
            let created_at = chrono::DateTime::parse_from_rfc3339(&timestamp)
                .map_err(|_| {
                    rusqlite::Error::InvalidColumnType(
                        3,
                        "created_at".to_string(),
                        rusqlite::types::Type::Text,
                    )
                })?
                .with_timezone(&Utc);
            Ok(JobLogEntry {
                sequence: row.get::<_, i64>(0)? as u64,
                stream: row.get(1)?,
                text: row.get(2)?,
                created_at,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
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

#[cfg(test)]
mod lease_tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn expired_running_job_can_be_reclaimed_but_live_lease_cannot() {
        let mut store = JobStore::in_memory().unwrap();
        let id = store
            .create("agent_operation", &json!({"operation": "system.info"}))
            .unwrap();
        let claimed_at = Utc::now();
        assert_eq!(
            store.claim_at("worker-1", claimed_at).unwrap().unwrap().id,
            id
        );

        let before_expiry = claimed_at + Duration::seconds(JOB_LEASE_TIMEOUT_SECONDS - 2);
        assert!(store.claim_at("worker-2", before_expiry).unwrap().is_none());

        let after_expiry = claimed_at + Duration::seconds(JOB_LEASE_TIMEOUT_SECONDS + 2);
        let reclaimed = store.claim_at("worker-2", after_expiry).unwrap().unwrap();
        assert_eq!(reclaimed.id, id);
        assert_eq!(reclaimed.worker_id.as_deref(), Some("worker-2"));
    }
}

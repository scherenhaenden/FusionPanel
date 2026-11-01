#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

/// Maximum UTF-8 byte length stored in a job error message.
pub const MAX_ERROR_TEXT_BYTES: usize = 4096;

/// Unique identifier for a job.
pub type JobId = String;

/// The state of a job in the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

impl JobState {
    pub fn as_str(&self) -> &'static str {
        match self {
            JobState::Queued => "queued",
            JobState::Running => "running",
            JobState::Succeeded => "succeeded",
            JobState::Failed => "failed",
            JobState::Cancelled => "cancelled",
        }
    }
}

impl std::str::FromStr for JobState {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "queued" => Ok(JobState::Queued),
            "running" => Ok(JobState::Running),
            "succeeded" => Ok(JobState::Succeeded),
            "failed" => Ok(JobState::Failed),
            "cancelled" => Ok(JobState::Cancelled),
            _ => Err(()),
        }
    }
}

/// A job record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Job {
    pub id: JobId,
    pub kind: String,
    pub payload: Value,
    pub state: JobState,
    pub worker_id: Option<String>,
    pub error_text: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Errors that can occur during job store operations.
#[derive(Debug, Error)]
pub enum JobStoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Job not found: {0}")]
    NotFound(JobId),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: JobState, to: JobState },

    #[error("Malformed data in database: {0}")]
    MalformedData(String),

    #[error("Job error text is {actual_bytes} bytes; maximum is {max_bytes} bytes")]
    ErrorTextTooLong {
        max_bytes: usize,
        actual_bytes: usize,
    },
}

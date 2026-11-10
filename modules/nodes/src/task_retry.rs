#![forbid(unsafe_code)]

use std::time::Duration;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ErrorClassification {
    Transient,
    Fatal,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TaskError {
    pub message: String,
    pub classification: ErrorClassification,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum TaskState {
    Pending,
    InProgress,
    Completed,
    Retrying { attempt: u32, next_retry_at: DateTime<Utc> },
    DeadLetterQueue { error: TaskError },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub state: TaskState,
    pub max_retries: u32,
    pub base_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl Task {
    pub fn new(id: String, max_retries: u32, base_backoff_ms: u64, max_backoff_ms: u64) -> Self {
        Self {
            id,
            state: TaskState::Pending,
            max_retries,
            base_backoff_ms,
            max_backoff_ms,
        }
    }

    pub fn start(&mut self) {
        if let TaskState::Pending | TaskState::Retrying { .. } = self.state {
            self.state = TaskState::InProgress;
        }
    }

    pub fn complete(&mut self) {
        if matches!(self.state, TaskState::InProgress) {
            self.state = TaskState::Completed;
        }
    }

    pub fn fail(&mut self, error: TaskError, attempt: u32, now: DateTime<Utc>, jitter_factor: f64) {
        if !matches!(self.state, TaskState::InProgress) {
            return;
        }

        match error.classification {
            ErrorClassification::Fatal => {
                self.state = TaskState::DeadLetterQueue { error };
            }
            ErrorClassification::Transient => {
                if attempt >= self.max_retries {
                    self.state = TaskState::DeadLetterQueue { error };
                } else {
                    let backoff = self.calculate_backoff(attempt, jitter_factor);
                    self.state = TaskState::Retrying {
                        attempt: attempt + 1,
                        next_retry_at: now + chrono::Duration::from_std(backoff).unwrap_or(chrono::Duration::zero()),
                    };
                }
            }
        }
    }

    fn calculate_backoff(&self, attempt: u32, jitter_factor: f64) -> Duration {
        let max_backoff = Duration::from_millis(self.max_backoff_ms);
        let base_backoff = Duration::from_millis(self.base_backoff_ms);

        let bounded_attempt = attempt.min(127);
        let temp = base_backoff.as_millis() as u128 * 2u128.pow(bounded_attempt);
        let temp_duration = if temp > max_backoff.as_millis() as u128 {
            max_backoff
        } else {
            Duration::from_millis(temp as u64)
        };

        // Full jitter calculation: random between 0 and temp_duration
        let jittered_millis = (temp_duration.as_millis() as f64 * jitter_factor) as u64;

        Duration::from_millis(jittered_millis)
    }
}

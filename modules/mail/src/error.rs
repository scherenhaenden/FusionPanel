use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub enum MailError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Resource conflict: {0}")]
    Conflict(String),

    #[error("Unsupported operation or configuration: {0}")]
    Unsupported(String),

    #[error("Dependency unavailable: {0}")]
    DependencyUnavailable(String),

    #[error("Operation timed out: {0}")]
    Timeout(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Reconciliation required: {0}")]
    ReconciliationRequired(String),
}

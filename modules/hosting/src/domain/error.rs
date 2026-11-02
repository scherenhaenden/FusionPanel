use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Unsupported: {0}")]
    Unsupported(String),
    #[error("Dependency unavailable: {0}")]
    DependencyUnavailable(String),
    #[error("Timeout: {0}")]
    Timeout(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Reconciliation required: {0}")]
    ReconciliationRequired(String),
}

pub type Result<T> = std::result::Result<T, DomainError>;

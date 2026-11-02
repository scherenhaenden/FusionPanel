use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DeploymentError {
    #[error("invalid input: {0}")]
    InvalidInput(String),

    #[error("unauthorized: {0}")]
    Unauthorized(String),

    #[error("conflict: {0}")]
    Conflict(String),

    #[error("unsupported mode or capability: {0}")]
    Unsupported(String),

    #[error("dependency unavailable: {0}")]
    DependencyUnavailable(String),

    #[error("operation failed: {0}")]
    OperationFailed(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("reconciliation required: {0}")]
    ReconciliationRequired(String),

    #[error("signature mismatch: {0}")]
    SignatureMismatch(String),

    #[error("replay detected: {0}")]
    ReplayDetected(String),

    #[error("not found: {0}")]
    NotFound(String),
}

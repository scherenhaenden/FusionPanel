use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IdentityError {
    #[error("Invalid input: {message}")]
    InvalidInput { message: String },

    #[error("Unauthorized: {reason}")]
    Unauthorized { reason: String },

    #[error("Resource conflict: {message}")]
    Conflict { message: String },

    #[error("Resource not found: {resource} {id}")]
    NotFound { resource: String, id: String },

    #[error("Capability or feature unsupported: {capability}")]
    Unsupported { capability: String },

    #[error("Session expired")]
    SessionExpired,

    #[error("Session revoked: {reason}")]
    SessionRevoked { reason: String },

    #[error("Password verification failed")]
    InvalidCredentials,

    #[error("Storage error: {message}")]
    Storage { message: String },

    #[error("Operation failed: {message}")]
    OperationFailed { message: String },
}

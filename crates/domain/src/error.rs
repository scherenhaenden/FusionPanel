use crate::state::ResourceState;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("Invalid ID format: {0}")]
    InvalidId(String),

    #[error("Invalid domain name: {0}")]
    InvalidDomainName(String),

    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidStateTransition {
        from: ResourceState,
        to: ResourceState,
    },
}

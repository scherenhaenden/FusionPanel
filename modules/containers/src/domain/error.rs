use thiserror::Error;

#[derive(Debug, Error, PartialEq)]
pub enum DomainError {
    #[error("Invalid container ID format")]
    InvalidContainerId,
    #[error("Invalid image reference format")]
    InvalidImageRef,
    #[error("Invalid port range: host_port {0} out of bounds")]
    InvalidHostPort(u16),
    #[error("Invalid port range: container_port {0} out of bounds")]
    InvalidContainerPort(u16),
    #[error("Invalid transition from {from:?} to {to:?}")]
    InvalidStatusTransition {
        from: super::ContainerStatus,
        to: super::ContainerStatus,
    },
    #[error("Invalid mount point: {0}")]
    InvalidMountPoint(String),
}

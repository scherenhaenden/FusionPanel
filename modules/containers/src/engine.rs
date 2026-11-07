use async_trait::async_trait;

use crate::domain::DomainError;
use crate::domain::{ContainerId, ContainerSpec, ContainerStatus};

#[async_trait]
pub trait ContainerEngine: Send + Sync {
    async fn create(&self, spec: &ContainerSpec) -> Result<(), DomainError>;
    async fn start(&self, id: &ContainerId) -> Result<(), DomainError>;
    async fn stop(&self, id: &ContainerId) -> Result<(), DomainError>;
    async fn restart(&self, id: &ContainerId) -> Result<(), DomainError>;
    async fn remove(&self, id: &ContainerId) -> Result<(), DomainError>;
    async fn inspect(&self, id: &ContainerId) -> Result<ContainerStatus, DomainError>;
}

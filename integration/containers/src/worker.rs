#![forbid(unsafe_code)]

use crate::ports::PortAllocator;
use crate::store::ContainerStore;
use containers::docker::adapter::{
    ApplyOutcome, BoundedLogs, DesiredContainer, DockerAdapterError, NodeDockerAdapter,
    ReconcileReport, RemoveOutcome,
};
use containers::domain::{ContainerId, ContainerSpec, ContainerStatus};
use std::sync::Arc;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContainerWorkerError {
    #[error("container '{0}' is not managed by this node")]
    UnmanagedContainer(String),
    #[error("container name collision for '{0}'")]
    NameCollision(String),
    #[error("container '{0}' not found")]
    NotFound(String),
    #[error("invalid container request: {0}")]
    InvalidRequest(String),
    #[error("access denied: {0}")]
    AccessDenied(String),
    #[error("container engine failure: {0}")]
    EngineFailure(String),
    #[error("persistence failure: {0}")]
    StoreFailure(String),
}

impl From<DockerAdapterError> for ContainerWorkerError {
    fn from(err: DockerAdapterError) -> Self {
        match err {
            DockerAdapterError::UnmanagedContainer(id) => Self::UnmanagedContainer(id),
            DockerAdapterError::NameCollision(id) => Self::NameCollision(id),
            DockerAdapterError::NotFound(id) => Self::NotFound(id),
            DockerAdapterError::InvalidNodeId(msg) | DockerAdapterError::Domain(msg) => {
                Self::InvalidRequest(msg)
            }
            DockerAdapterError::Engine(msg) => Self::EngineFailure(msg),
        }
    }
}

pub struct ContainerWorkerService {
    adapter: Arc<NodeDockerAdapter>,
    store: Arc<dyn ContainerStore>,
    port_allocator: Arc<PortAllocator>,
}

impl ContainerWorkerService {
    pub fn new(
        adapter: Arc<NodeDockerAdapter>,
        store: Arc<dyn ContainerStore>,
        port_allocator: Arc<PortAllocator>,
    ) -> Self {
        Self {
            adapter,
            store,
            port_allocator,
        }
    }

    pub fn node_id(&self) -> &str {
        self.adapter.node_id()
    }

    fn verify_tenant_ownership(
        &self,
        tenant_id: &str,
        container_id: &str,
    ) -> Result<(), ContainerWorkerError> {
        match self.store.get_container_tenant(container_id) {
            Ok(Some(owner)) if owner == tenant_id => Ok(()),
            Ok(Some(_)) => Err(ContainerWorkerError::AccessDenied(
                "container belongs to another tenant".to_string(),
            )),
            Ok(None) => Err(ContainerWorkerError::NotFound(container_id.to_string())),
            Err(err) => Err(ContainerWorkerError::StoreFailure(err.to_string())),
        }
    }

    pub async fn apply_container(
        &self,
        tenant_id: &str,
        spec: &ContainerSpec,
        desired_running: bool,
    ) -> Result<ApplyOutcome, ContainerWorkerError> {
        let id_str = spec.id.as_str();
        let mut newly_recorded = false;
        let ports: Vec<u16> = spec.ports.iter().map(|p| p.host_port()).collect();

        match self.store.get_container_tenant(id_str) {
            Ok(Some(owner)) if owner != tenant_id => {
                return Err(ContainerWorkerError::AccessDenied(
                    "container belongs to another tenant".to_string(),
                ));
            }
            Ok(Some(_)) => {}
            Ok(None) => {
                self.store
                    .create_container(id_str, tenant_id, id_str, &ports)
                    .map_err(|e| ContainerWorkerError::StoreFailure(e.to_string()))?;
                newly_recorded = true;
            }
            Err(err) => return Err(ContainerWorkerError::StoreFailure(err.to_string())),
        }

        match self.adapter.apply(spec, Some(tenant_id), desired_running).await {
            Ok(outcome) => Ok(outcome),
            Err(err) => {
                if newly_recorded {
                    let _ = self.store.remove_container(id_str);
                    for port in &ports {
                        self.port_allocator.release(*port);
                    }
                }
                Err(err.into())
            }
        }
    }

    pub async fn start_container(
        &self,
        tenant_id: &str,
        id: &ContainerId,
    ) -> Result<(), ContainerWorkerError> {
        self.verify_tenant_ownership(tenant_id, id.as_str())?;
        self.adapter.start(id).await.map_err(Into::into)
    }

    pub async fn stop_container(
        &self,
        tenant_id: &str,
        id: &ContainerId,
    ) -> Result<(), ContainerWorkerError> {
        self.verify_tenant_ownership(tenant_id, id.as_str())?;
        self.adapter.stop(id).await.map_err(Into::into)
    }

    pub async fn restart_container(
        &self,
        tenant_id: &str,
        id: &ContainerId,
    ) -> Result<(), ContainerWorkerError> {
        self.verify_tenant_ownership(tenant_id, id.as_str())?;
        self.adapter.restart(id).await.map_err(Into::into)
    }

    pub async fn remove_container(
        &self,
        tenant_id: &str,
        id: &ContainerId,
    ) -> Result<RemoveOutcome, ContainerWorkerError> {
        self.verify_tenant_ownership(tenant_id, id.as_str())?;
        let outcome = self.adapter.remove(id).await?;
        if let Ok(ports) = self.store.get_container_ports(id.as_str()) {
            for port in ports {
                self.port_allocator.release(port);
            }
        }
        let _ = self.store.remove_container(id.as_str());
        Ok(outcome)
    }

    pub async fn inspect_container(
        &self,
        tenant_id: &str,
        id: &ContainerId,
    ) -> Result<ContainerStatus, ContainerWorkerError> {
        self.verify_tenant_ownership(tenant_id, id.as_str())?;
        self.adapter.inspect_owned(id).await.map_err(Into::into)
    }

    pub async fn fetch_logs(
        &self,
        tenant_id: &str,
        id: &ContainerId,
        max_lines: usize,
        max_bytes: usize,
    ) -> Result<BoundedLogs, ContainerWorkerError> {
        self.verify_tenant_ownership(tenant_id, id.as_str())?;
        self.adapter
            .logs(id, max_lines, max_bytes)
            .await
            .map_err(Into::into)
    }

    pub async fn reconcile(
        &self,
        desired: &[DesiredContainer],
    ) -> Result<ReconcileReport, ContainerWorkerError> {
        self.adapter.reconcile(desired).await.map_err(Into::into)
    }
}

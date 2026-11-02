use serde::{Deserialize, Serialize};

use super::error::DomainError;
use super::id::ContainerId;
use super::image::ImageRef;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortMapping {
    host_port: u16,
    container_port: u16,
}

impl PortMapping {
    pub fn new(host_port: u16, container_port: u16) -> Result<Self, DomainError> {
        if host_port == 0 {
            return Err(DomainError::InvalidHostPort(host_port));
        }
        if container_port == 0 {
            return Err(DomainError::InvalidContainerPort(container_port));
        }
        Ok(Self {
            host_port,
            container_port,
        })
    }

    pub fn host_port(&self) -> u16 {
        self.host_port
    }

    pub fn container_port(&self) -> u16 {
        self.container_port
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MountPoint {
    source: String,
    target: String,
    read_only: bool,
}

impl MountPoint {
    pub fn new(source: String, target: String, read_only: bool) -> Result<Self, DomainError> {
        if source.is_empty() {
            return Err(DomainError::InvalidMountPoint(
                "Source path cannot be empty".to_string(),
            ));
        }
        if target.is_empty() {
            return Err(DomainError::InvalidMountPoint(
                "Target path cannot be empty".to_string(),
            ));
        }
        Ok(Self {
            source,
            target,
            read_only,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSpec {
    pub id: ContainerId,
    pub image: ImageRef,
    pub ports: Vec<PortMapping>,
    pub mounts: Vec<MountPoint>,
}

impl ContainerSpec {
    pub fn new(
        id: ContainerId,
        image: ImageRef,
        ports: Vec<PortMapping>,
        mounts: Vec<MountPoint>,
    ) -> Self {
        Self {
            id,
            image,
            ports,
            mounts,
        }
    }
}

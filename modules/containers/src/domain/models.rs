use serde::{Deserialize, Serialize};

use std::path::{Component, Path};

use super::error::DomainError;
use super::id::ContainerId;
use super::image::ImageRef;
use crate::resources::ResourceLimits;

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

        // Validate source path against sensitive host paths and traversals
        let mut normalized_path = std::path::PathBuf::new();
        for component in Path::new(&source).components() {
            match component {
                Component::ParentDir => {
                    normalized_path.pop();
                }
                Component::CurDir => {}
                _ => normalized_path.push(component),
            }
        }
        let host_path = normalized_path.to_string_lossy().into_owned();

        if host_path == "/" || host_path == "\\" || host_path.is_empty() {
            return Err(DomainError::InvalidMountPoint(
                "Root mount bypass is not allowed".to_string(),
            ));
        }

        let sensitive_paths = [
            "/",
            "/etc",
            "/var/run/docker.sock",
            "/bin",
            "/sbin",
            "/usr",
            "/sys",
            "/dev",
            "/var/lib/docker",
        ];

        for p in sensitive_paths {
            if host_path == *p || host_path.starts_with(&format!("{}/", p)) {
                return Err(DomainError::InvalidMountPoint(
                    format!("Mounting sensitive path {} is not allowed", p),
                ));
            }
        }

        Ok(Self {
            source,
            target,
            read_only,
        })
    }

    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn read_only(&self) -> bool {
        self.read_only
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerSpec {
    pub id: ContainerId,
    pub image: ImageRef,
    pub ports: Vec<PortMapping>,
    pub mounts: Vec<MountPoint>,
    pub network: Option<String>,
    pub limits: Option<ResourceLimits>,
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
            network: None,
            limits: None,
        }
    }

    pub fn with_network(mut self, network: String) -> Self {
        self.network = Some(network);
        self
    }

    pub fn with_limits(mut self, limits: ResourceLimits) -> Self {
        self.limits = Some(limits);
        self
    }
}

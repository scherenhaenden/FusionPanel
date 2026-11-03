use async_trait::async_trait;
use std::process::Command;

use crate::domain::{ContainerId, ContainerSpec, ContainerStatus, DomainError};
use crate::engine::ContainerEngine;

pub struct DockerCliEngine;

impl DockerCliEngine {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl ContainerEngine for DockerCliEngine {
    async fn create(&self, spec: &ContainerSpec) -> Result<(), DomainError> {
        let mut cmd = Command::new("docker");
        cmd.arg("create");
        cmd.arg("--name").arg(spec.id.as_str());

        // Limits
        if let Some(limits) = &spec.limits {
            // docker cpu-quota is in microseconds, and 100,000 is 1 core (which is 1000 millicores)
            // wait, simpler is to use --cpus.
            // but docker CLI --cpus accepts a float.
            // 1000 millicores = 1.0 CPUs.
            let cpus = (limits.cpu_millicores as f64) / 1000.0;
            cmd.arg(format!("--cpus={}", cpus));
            cmd.arg(format!("--memory={}b", limits.memory_bytes));
        }

        // Network
        if let Some(network) = &spec.network {
            cmd.arg("--network").arg(network);
        }

        // Ports
        for port in &spec.ports {
            cmd.arg("-p").arg(format!("{}:{}", port.host_port(), port.container_port()));
        }

        // Mounts
        for mount in &spec.mounts {
            let ro_str = if mount.read_only() { "ro" } else { "rw" };
            cmd.arg("-v").arg(format!("{}:{}:{}", mount.source(), mount.target(), ro_str));
        }

        cmd.arg(spec.image.as_str());

        let output = cmd.output().map_err(|e| DomainError::EngineError(e.to_string()))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::EngineError(format!("docker create failed: {}", err)));
        }

        Ok(())
    }

    async fn start(&self, id: &ContainerId) -> Result<(), DomainError> {
        let mut cmd = Command::new("docker");
        cmd.arg("start").arg(id.as_str());

        let output = cmd.output().map_err(|e| DomainError::EngineError(e.to_string()))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::EngineError(format!("docker start failed: {}", err)));
        }

        Ok(())
    }

    async fn stop(&self, id: &ContainerId) -> Result<(), DomainError> {
        let mut cmd = Command::new("docker");
        cmd.arg("stop").arg("-t").arg("10").arg(id.as_str());

        let output = cmd.output().map_err(|e| DomainError::EngineError(e.to_string()))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::EngineError(format!("docker stop failed: {}", err)));
        }

        Ok(())
    }

    async fn remove(&self, id: &ContainerId) -> Result<(), DomainError> {
        let mut cmd = Command::new("docker");
        cmd.arg("rm").arg("-f").arg("-v").arg(id.as_str());

        let output = cmd.output().map_err(|e| DomainError::EngineError(e.to_string()))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::EngineError(format!("docker rm failed: {}", err)));
        }

        Ok(())
    }

    async fn inspect(&self, id: &ContainerId) -> Result<ContainerStatus, DomainError> {
        let mut cmd = Command::new("docker");
        cmd.arg("inspect")
           .arg("--format={{.State.Status}}")
           .arg(id.as_str());

        let output = cmd.output().map_err(|e| DomainError::EngineError(e.to_string()))?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(DomainError::EngineError(format!("docker inspect failed: {}", err)));
        }

        let status_str = String::from_utf8_lossy(&output.stdout).trim().to_string();

        match status_str.as_str() {
            "created" => Ok(ContainerStatus::Created),
            "running" => Ok(ContainerStatus::Running),
            "exited" | "dead" => Ok(ContainerStatus::Exited),
            "paused" | "restarting" => Ok(ContainerStatus::Running), // simplify
            "" => Ok(ContainerStatus::Removed), // might happen if inspect fails? Usually rm succeeds so it's gone
            _ => Err(DomainError::EngineError(format!("Unknown state: {}", status_str))),
        }
    }
}

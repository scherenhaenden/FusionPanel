use async_trait::async_trait;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;
use tokio::time::timeout;

use crate::domain::{ContainerId, ContainerSpec, ContainerStatus, DomainError};
use crate::engine::ContainerEngine;

const DOCKER_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerCommandOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait DockerCommandRunner: Send + Sync {
    async fn run(&self, args: &[String]) -> Result<DockerCommandOutput, DomainError>;
}

#[derive(Debug, Default)]
pub struct SystemDockerRunner;

#[async_trait]
impl DockerCommandRunner for SystemDockerRunner {
    async fn run(&self, args: &[String]) -> Result<DockerCommandOutput, DomainError> {
        let mut cmd = Command::new("docker");
        cmd.args(args);

        let subcommand = args.first().map(String::as_str).unwrap_or("command");
        let output = timeout(DOCKER_TIMEOUT, cmd.output())
            .await
            .map_err(|_| DomainError::EngineError(format!("docker {subcommand} timed out")))?
            .map_err(|e| DomainError::EngineError(e.to_string()))?;

        Ok(DockerCommandOutput {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContainerInspectInfo {
    pub status: ContainerStatus,
    pub labels: BTreeMap<String, String>,
}

#[derive(Deserialize)]
struct RawInspectRecord {
    #[serde(rename = "State")]
    state: RawInspectState,
    #[serde(rename = "Config")]
    config: RawInspectConfig,
}

#[derive(Deserialize)]
struct RawInspectState {
    #[serde(rename = "Status")]
    status: String,
}

#[derive(Deserialize)]
struct RawInspectConfig {
    #[serde(rename = "Labels", default)]
    labels: Option<BTreeMap<String, String>>,
}

pub struct DockerCliEngine {
    runner: Arc<dyn DockerCommandRunner>,
}

impl Default for DockerCliEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl DockerCliEngine {
    pub fn new() -> Self {
        Self {
            runner: Arc::new(SystemDockerRunner),
        }
    }

    pub fn with_runner(runner: Arc<dyn DockerCommandRunner>) -> Self {
        Self { runner }
    }

    fn parse_status(status_str: &str) -> Result<ContainerStatus, DomainError> {
        match status_str.trim() {
            "created" => Ok(ContainerStatus::Created),
            "running" => Ok(ContainerStatus::Running),
            "exited" | "dead" => Ok(ContainerStatus::Exited),
            "paused" | "restarting" => Ok(ContainerStatus::Running),
            "" => Ok(ContainerStatus::Removed),
            other => Err(DomainError::EngineError(format!("Unknown state: {other}"))),
        }
    }

    fn is_not_found_error(stderr: &str) -> bool {
        let lower = stderr.to_ascii_lowercase();
        lower.contains("no such object")
            || lower.contains("no such container")
            || lower.contains("not found")
    }

    pub async fn inspect_details(
        &self,
        id: &ContainerId,
    ) -> Result<Option<ContainerInspectInfo>, DomainError> {
        let args = vec![
            "inspect".to_string(),
            "--format={{json .}}".to_string(),
            id.as_str().to_string(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            if Self::is_not_found_error(&output.stderr) {
                return Ok(None);
            }
            return Err(DomainError::EngineError(format!(
                "docker inspect failed: {}",
                output.stderr
            )));
        }

        let trimmed = output.stdout.trim();
        if trimmed.is_empty() {
            return Ok(None);
        }

        let record: RawInspectRecord = serde_json::from_str(trimmed).map_err(|e| {
            DomainError::EngineError(format!("failed to parse docker inspect output: {e}"))
        })?;
        let status = Self::parse_status(&record.state.status)?;
        let labels = record.config.labels.unwrap_or_default();
        Ok(Some(ContainerInspectInfo { status, labels }))
    }

    pub async fn list_by_labels(
        &self,
        filters: &[(&str, &str)],
    ) -> Result<Vec<ContainerId>, DomainError> {
        let mut args = vec!["ps".to_string(), "-a".to_string()];
        for (key, value) in filters {
            args.push("--filter".to_string());
            args.push(format!("label={key}={value}"));
        }
        args.push("--format={{.Names}}".to_string());

        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker ps failed: {}",
                output.stderr
            )));
        }

        let mut ids = Vec::new();
        for line in output.stdout.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(id) = ContainerId::new(trimmed.to_string()) {
                ids.push(id);
            }
        }
        Ok(ids)
    }

    pub async fn logs(&self, id: &ContainerId, tail: usize) -> Result<Vec<String>, DomainError> {
        let args = vec![
            "logs".to_string(),
            "--tail".to_string(),
            tail.to_string(),
            id.as_str().to_string(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker logs failed: {}",
                output.stderr
            )));
        }

        let mut lines: Vec<String> = output.stdout.lines().map(ToString::to_string).collect();
        for line in output.stderr.lines() {
            if !line.trim().is_empty() {
                lines.push(line.to_string());
            }
        }
        Ok(lines)
    }
}

#[async_trait]
impl ContainerEngine for DockerCliEngine {
    async fn create(&self, spec: &ContainerSpec) -> Result<(), DomainError> {
        let mut args = vec![
            "create".to_string(),
            "--name".to_string(),
            spec.id.as_str().to_string(),
        ];

        // Limits
        if let Some(limits) = &spec.limits {
            let cpus = (limits.cpu_millicores as f64) / 1000.0;
            args.push(format!("--cpus={}", cpus));
            args.push(format!("--memory={}b", limits.memory_bytes));
        }

        // Network
        if let Some(network) = &spec.network {
            args.push("--network".to_string());
            args.push(network.clone());
        }

        // Restart policy
        if let Some(policy) = &spec.restart_policy {
            args.push("--restart".to_string());
            args.push(policy.clone());
        }

        // Ports
        for port in &spec.ports {
            args.push("-p".to_string());
            args.push(format!("{}:{}", port.host_port(), port.container_port()));
        }

        // Mounts
        for mount in &spec.mounts {
            let ro_str = if mount.read_only() { "ro" } else { "rw" };
            args.push("-v".to_string());
            args.push(format!("{}:{}:{}", mount.source(), mount.target(), ro_str));
        }

        // Environment variables
        for (key, value) in &spec.env {
            args.push("-e".to_string());
            args.push(format!("{key}={value}"));
        }

        // Labels
        for (key, value) in &spec.labels {
            args.push("--label".to_string());
            args.push(format!("{key}={value}"));
        }

        args.push(spec.image.as_str().to_string());

        // Optional command
        for arg in &spec.command {
            args.push(arg.clone());
        }

        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker create failed: {}",
                output.stderr
            )));
        }

        Ok(())
    }

    async fn start(&self, id: &ContainerId) -> Result<(), DomainError> {
        let args = vec!["start".to_string(), id.as_str().to_string()];
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker start failed: {}",
                output.stderr
            )));
        }
        Ok(())
    }

    async fn stop(&self, id: &ContainerId) -> Result<(), DomainError> {
        let args = vec![
            "stop".to_string(),
            "-t".to_string(),
            "10".to_string(),
            id.as_str().to_string(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker stop failed: {}",
                output.stderr
            )));
        }
        Ok(())
    }

    async fn restart(&self, id: &ContainerId) -> Result<(), DomainError> {
        let args = vec![
            "restart".to_string(),
            "-t".to_string(),
            "10".to_string(),
            id.as_str().to_string(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker restart failed: {}",
                output.stderr
            )));
        }
        Ok(())
    }

    async fn remove(&self, id: &ContainerId) -> Result<(), DomainError> {
        let args = vec![
            "rm".to_string(),
            "-f".to_string(),
            "-v".to_string(),
            id.as_str().to_string(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker rm failed: {}",
                output.stderr
            )));
        }
        Ok(())
    }

    async fn inspect(&self, id: &ContainerId) -> Result<ContainerStatus, DomainError> {
        let args = vec![
            "inspect".to_string(),
            "--format={{.State.Status}}".to_string(),
            id.as_str().to_string(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(DomainError::EngineError(format!(
                "docker inspect failed: {}",
                output.stderr
            )));
        }
        Self::parse_status(&output.stdout)
    }
}

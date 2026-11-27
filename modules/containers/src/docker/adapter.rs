use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use thiserror::Error;

use crate::docker::engine::DockerCliEngine;
use crate::domain::{ContainerId, ContainerSpec, ContainerStatus, DomainError};
use crate::engine::ContainerEngine;
use crate::logs::BoundedLogReader;

pub const LABEL_MANAGED: &str = "fusionpanel.managed";
pub const LABEL_NODE_ID: &str = "fusionpanel.node_id";
pub const LABEL_CONTAINER_ID: &str = "fusionpanel.container_id";
pub const LABEL_TENANT_ID: &str = "fusionpanel.tenant_id";
pub const LABEL_SPEC_HASH: &str = "fusionpanel.spec_hash";

pub const DEFAULT_MAX_LOG_LINES: usize = 200;
pub const HARD_MAX_LOG_LINES: usize = 1000;
pub const DEFAULT_MAX_LOG_BYTES: usize = 16 * 1024;
pub const HARD_MAX_LOG_BYTES: usize = 64 * 1024;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DockerAdapterError {
    #[error("container '{0}' is not managed by this node")]
    UnmanagedContainer(String),
    #[error("container name collision with unmanaged or foreign container '{0}'")]
    NameCollision(String),
    #[error("managed container '{0}' not found")]
    NotFound(String),
    #[error("invalid node id '{0}'")]
    InvalidNodeId(String),
    #[error("domain validation error: {0}")]
    Domain(String),
    #[error("docker engine error: {0}")]
    Engine(String),
}

impl From<DomainError> for DockerAdapterError {
    fn from(err: DomainError) -> Self {
        match err {
            DomainError::EngineError(msg) => Self::Engine(msg),
            other => Self::Domain(other.to_string()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplyOutcome {
    Created,
    AlreadyRunning,
    StartedExisting,
    StoppedExisting,
    AlreadyStopped,
    Recreated,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RemoveOutcome {
    Removed,
    NotFound,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DesiredContainer {
    pub spec: ContainerSpec,
    pub tenant_id: Option<String>,
    pub desired_running: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ReconcileReport {
    pub applied: Vec<String>,
    pub removed_orphans: Vec<String>,
    pub skipped_unmanaged: Vec<String>,
    pub errors: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedLogs {
    pub lines: Vec<String>,
    pub truncated: bool,
    pub total_bytes: usize,
}

/// Node-scoped Docker adapter that enforces ownership labels, handles name collisions
/// without duplicating or deleting unmanaged containers, supports restart reconciliation,
/// and provides bounded redacted log retrieval.
pub struct NodeDockerAdapter {
    node_id: String,
    engine: Arc<DockerCliEngine>,
}

impl NodeDockerAdapter {
    pub fn new(
        node_id: impl Into<String>,
        engine: Arc<DockerCliEngine>,
    ) -> Result<Self, DockerAdapterError> {
        let node_id = node_id.into();
        let trimmed = node_id.trim();
        if trimmed.is_empty()
            || trimmed.len() > 128
            || !trimmed
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(DockerAdapterError::InvalidNodeId(node_id));
        }
        Ok(Self {
            node_id: trimmed.to_string(),
            engine,
        })
    }

    pub fn node_id(&self) -> &str {
        &self.node_id
    }

    pub fn ownership_labels(
        &self,
        container_id: &ContainerId,
        tenant_id: Option<&str>,
        spec_hash: &str,
    ) -> BTreeMap<String, String> {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_MANAGED.to_string(), "true".to_string());
        labels.insert(LABEL_NODE_ID.to_string(), self.node_id.clone());
        labels.insert(
            LABEL_CONTAINER_ID.to_string(),
            container_id.as_str().to_string(),
        );
        labels.insert(LABEL_SPEC_HASH.to_string(), spec_hash.to_string());
        if let Some(tenant) = tenant_id.filter(|t| !t.trim().is_empty()) {
            labels.insert(LABEL_TENANT_ID.to_string(), tenant.trim().to_string());
        }
        labels
    }

    pub fn is_owned_by_node(
        &self,
        labels: &BTreeMap<String, String>,
        expected_container_id: Option<&ContainerId>,
    ) -> bool {
        let managed = labels.get(LABEL_MANAGED).map(String::as_str) == Some("true");
        let node_matches = labels.get(LABEL_NODE_ID).map(String::as_str) == Some(&self.node_id);
        if !managed || !node_matches {
            return false;
        }
        if let Some(expected) = expected_container_id {
            labels.get(LABEL_CONTAINER_ID).map(String::as_str) == Some(expected.as_str())
        } else {
            true
        }
    }

    /// Deterministic FNV-1a 64-bit hash over the spec fields that affect container creation.
    pub fn compute_spec_hash(spec: &ContainerSpec, tenant_id: Option<&str>) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        let mix = |h: &mut u64, bytes: &[u8]| {
            for &b in bytes {
                *h ^= u64::from(b);
                *h = h.wrapping_mul(0x100000001b3);
            }
            *h ^= 0xff;
            *h = h.wrapping_mul(0x100000001b3);
        };

        mix(&mut hash, spec.id.as_str().as_bytes());
        mix(&mut hash, spec.image.as_str().as_bytes());
        if let Some(t) = tenant_id {
            mix(&mut hash, t.as_bytes());
        }
        if let Some(net) = &spec.network {
            mix(&mut hash, net.as_bytes());
        }
        if let Some(policy) = &spec.restart_policy {
            mix(&mut hash, policy.as_bytes());
        }
        if let Some(limits) = &spec.limits {
            mix(&mut hash, &limits.cpu_millicores.to_le_bytes());
            mix(&mut hash, &limits.memory_bytes.to_le_bytes());
        }
        for port in &spec.ports {
            mix(&mut hash, &port.host_port().to_le_bytes());
            mix(&mut hash, &port.container_port().to_le_bytes());
        }
        for mount in &spec.mounts {
            mix(&mut hash, mount.source().as_bytes());
            mix(&mut hash, mount.target().as_bytes());
            mix(&mut hash, &[u8::from(mount.read_only())]);
        }
        for arg in &spec.command {
            mix(&mut hash, arg.as_bytes());
        }
        for (k, v) in &spec.env {
            mix(&mut hash, k.as_bytes());
            mix(&mut hash, v.as_bytes());
        }
        for (k, v) in &spec.labels {
            if !k.starts_with("fusionpanel.") {
                mix(&mut hash, k.as_bytes());
                mix(&mut hash, v.as_bytes());
            }
        }
        format!("{hash:016x}")
    }

    fn prepare_spec(&self, spec: &ContainerSpec, tenant_id: Option<&str>) -> (ContainerSpec, String) {
        let spec_hash = Self::compute_spec_hash(spec, tenant_id);
        let mut prepared = spec.clone();
        for (k, v) in self.ownership_labels(&spec.id, tenant_id, &spec_hash) {
            prepared.labels.insert(k, v);
        }
        (prepared, spec_hash)
    }

    async fn require_owned(&self, id: &ContainerId) -> Result<ContainerStatus, DockerAdapterError> {
        let Some(info) = self.engine.inspect_details(id).await? else {
            return Err(DockerAdapterError::NotFound(id.to_string()));
        };
        if !self.is_owned_by_node(&info.labels, Some(id)) {
            return Err(DockerAdapterError::UnmanagedContainer(id.to_string()));
        }
        Ok(info.status)
    }

    /// Idempotently applies a container spec on this node.
    /// - If an unmanaged or foreign container exists with the same name, fails with `NameCollision`
    ///   and never modifies or deletes it.
    /// - If a managed container owned by this node already exists with matching spec hash,
    ///   reconciles its run state without creating a duplicate container.
    /// - If a managed container owned by this node exists with a different spec hash,
    ///   recreates it cleanly.
    pub async fn apply(
        &self,
        spec: &ContainerSpec,
        tenant_id: Option<&str>,
        desired_running: bool,
    ) -> Result<ApplyOutcome, DockerAdapterError> {
        let (prepared, spec_hash) = self.prepare_spec(spec, tenant_id);

        if let Some(existing) = self.engine.inspect_details(&spec.id).await? {
            if !self.is_owned_by_node(&existing.labels, Some(&spec.id)) {
                return Err(DockerAdapterError::NameCollision(spec.id.to_string()));
            }

            let existing_hash = existing.labels.get(LABEL_SPEC_HASH).map(String::as_str);
            if existing_hash == Some(spec_hash.as_str()) {
                return if desired_running {
                    if existing.status == ContainerStatus::Running {
                        Ok(ApplyOutcome::AlreadyRunning)
                    } else {
                        self.engine.start(&spec.id).await?;
                        Ok(ApplyOutcome::StartedExisting)
                    }
                } else if existing.status == ContainerStatus::Running {
                    self.engine.stop(&spec.id).await?;
                    Ok(ApplyOutcome::StoppedExisting)
                } else {
                    Ok(ApplyOutcome::AlreadyStopped)
                };
            }

            self.engine.remove(&spec.id).await?;
            self.engine.create(&prepared).await?;
            if desired_running {
                self.engine.start(&spec.id).await?;
            }
            return Ok(ApplyOutcome::Recreated);
        }

        self.engine.create(&prepared).await?;
        if desired_running {
            self.engine.start(&spec.id).await?;
        }
        Ok(ApplyOutcome::Created)
    }

    pub async fn start(&self, id: &ContainerId) -> Result<(), DockerAdapterError> {
        let status = self.require_owned(id).await?;
        if status == ContainerStatus::Running {
            return Ok(());
        }
        self.engine.start(id).await?;
        Ok(())
    }

    pub async fn stop(&self, id: &ContainerId) -> Result<(), DockerAdapterError> {
        let status = self.require_owned(id).await?;
        if status != ContainerStatus::Running {
            return Ok(());
        }
        self.engine.stop(id).await?;
        Ok(())
    }

    pub async fn restart(&self, id: &ContainerId) -> Result<(), DockerAdapterError> {
        let _ = self.require_owned(id).await?;
        self.engine.restart(id).await?;
        Ok(())
    }

    pub async fn remove(&self, id: &ContainerId) -> Result<RemoveOutcome, DockerAdapterError> {
        let Some(info) = self.engine.inspect_details(id).await? else {
            return Ok(RemoveOutcome::NotFound);
        };
        if !self.is_owned_by_node(&info.labels, Some(id)) {
            return Err(DockerAdapterError::UnmanagedContainer(id.to_string()));
        }
        self.engine.remove(id).await?;
        Ok(RemoveOutcome::Removed)
    }

    pub async fn inspect_owned(&self, id: &ContainerId) -> Result<ContainerStatus, DockerAdapterError> {
        self.require_owned(id).await
    }

    pub async fn logs(
        &self,
        id: &ContainerId,
        max_lines: usize,
        max_bytes: usize,
    ) -> Result<BoundedLogs, DockerAdapterError> {
        let _ = self.require_owned(id).await?;
        let capped_lines = max_lines.clamp(1, HARD_MAX_LOG_LINES);
        let capped_bytes = max_bytes.clamp(1, HARD_MAX_LOG_BYTES);

        let raw_lines = self.engine.logs(id, capped_lines).await?;
        let mut reader = BoundedLogReader::new(capped_lines);
        for line in &raw_lines {
            reader.append_log(line);
        }

        let redacted_lines = reader.get_logs();
        let mut bounded_lines = Vec::new();
        let mut total_bytes = 0usize;
        let mut truncated = raw_lines.len() > capped_lines;

        for line in redacted_lines {
            let line_bytes = line.len();
            if total_bytes + line_bytes > capped_bytes {
                let remaining = capped_bytes.saturating_sub(total_bytes);
                if remaining > 0 {
                    let mut prefix = String::new();
                    for ch in line.chars() {
                        if prefix.len() + ch.len_utf8() > remaining {
                            break;
                        }
                        prefix.push(ch);
                    }
                    if !prefix.is_empty() {
                        total_bytes += prefix.len();
                        bounded_lines.push(prefix);
                    }
                }
                truncated = true;
                break;
            }
            total_bytes += line_bytes;
            bounded_lines.push(line);
        }

        Ok(BoundedLogs {
            lines: bounded_lines,
            truncated,
            total_bytes,
        })
    }

    /// Reconciles all managed containers on this node against `desired`.
    /// Unmanaged or sentinel containers are never listed, modified, or deleted.
    pub async fn reconcile(&self, desired: &[DesiredContainer]) -> Result<ReconcileReport, DockerAdapterError> {
        let mut report = ReconcileReport::default();
        let desired_ids: BTreeSet<String> = desired
            .iter()
            .map(|item| item.spec.id.as_str().to_string())
            .collect();

        for item in desired {
            match self
                .apply(&item.spec, item.tenant_id.as_deref(), item.desired_running)
                .await
            {
                Ok(_) => report.applied.push(item.spec.id.to_string()),
                Err(err) => report
                    .errors
                    .push((item.spec.id.to_string(), err.to_string())),
            }
        }

        let managed_on_node = self
            .engine
            .list_by_labels(&[(LABEL_MANAGED, "true"), (LABEL_NODE_ID, &self.node_id)])
            .await?;

        for existing_id in managed_on_node {
            if desired_ids.contains(existing_id.as_str()) {
                continue;
            }
            match self.remove(&existing_id).await {
                Ok(RemoveOutcome::Removed) => report.removed_orphans.push(existing_id.to_string()),
                Ok(RemoveOutcome::NotFound) => {}
                Err(DockerAdapterError::UnmanagedContainer(id)) => {
                    report.skipped_unmanaged.push(id);
                }
                Err(err) => report.errors.push((existing_id.to_string(), err.to_string())),
            }
        }

        Ok(report)
    }
}

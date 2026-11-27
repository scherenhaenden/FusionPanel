#![forbid(unsafe_code)]

use crate::executor::{CommandExecutor, CommandRequest};
use crate::worker::sanitize_bounded;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

pub const LABEL_MANAGED: &str = "fusionpanel.managed";
pub const LABEL_NODE_ID: &str = "fusionpanel.node_id";
pub const LABEL_CONTAINER_ID: &str = "fusionpanel.container_id";
pub const LABEL_TENANT_ID: &str = "fusionpanel.tenant_id";
pub const LABEL_SPEC_HASH: &str = "fusionpanel.spec_hash";

const DOCKER_TIMEOUT: Duration = Duration::from_secs(30);
pub const DEFAULT_MAX_LOG_LINES: usize = 200;
pub const HARD_MAX_LOG_LINES: usize = 1000;
pub const DEFAULT_MAX_LOG_BYTES: usize = 4096;
pub const HARD_MAX_LOG_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerPortSpec {
    pub host_port: u16,
    pub container_port: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerMountSpec {
    pub source: String,
    pub target: String,
    #[serde(default)]
    pub read_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerApplyPayload {
    pub container_id: String,
    #[serde(default)]
    pub tenant_id: Option<Uuid>,
    pub image: String,
    #[serde(default)]
    pub ports: Vec<ContainerPortSpec>,
    #[serde(default)]
    pub mounts: Vec<ContainerMountSpec>,
    #[serde(default)]
    pub network: Option<String>,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub restart_policy: Option<String>,
    #[serde(default = "default_true")]
    pub desired_running: bool,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerLifecyclePayload {
    pub container_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContainerLogsPayload {
    pub container_id: String,
    #[serde(default = "default_log_lines")]
    pub max_lines: usize,
    #[serde(default = "default_log_bytes")]
    pub max_bytes: usize,
}

fn default_log_lines() -> usize {
    DEFAULT_MAX_LOG_LINES
}

fn default_log_bytes() -> usize {
    DEFAULT_MAX_LOG_BYTES
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContainerDriverError {
    UnmanagedContainer(String),
    NameCollision(String),
    NotFound(String),
    InvalidInput(String),
    Engine(String),
}

impl std::fmt::Display for ContainerDriverError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnmanagedContainer(id) => {
                write!(f, "container '{id}' is not managed by this node")
            }
            Self::NameCollision(id) => {
                write!(f, "container name collision with unmanaged container '{id}'")
            }
            Self::NotFound(id) => write!(f, "managed container '{id}' not found"),
            Self::InvalidInput(msg) => write!(f, "invalid container request: {msg}"),
            Self::Engine(msg) => write!(f, "container engine error: {msg}"),
        }
    }
}

impl std::error::Error for ContainerDriverError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainerApplyOutcome {
    Created,
    AlreadyRunning,
    StartedExisting,
    StoppedExisting,
    AlreadyStopped,
    Recreated,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ContainerReconcileReport {
    pub applied: Vec<String>,
    pub removed_orphans: Vec<String>,
    pub skipped_unmanaged: Vec<String>,
    pub errors: Vec<(String, String)>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedContainerLogs {
    pub lines: Vec<String>,
    pub truncated: bool,
    pub total_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DockerOutput {
    pub success: bool,
    pub stdout: String,
    pub stderr: String,
}

#[async_trait]
pub trait DockerRunner: Send + Sync {
    async fn run(&self, args: &[String]) -> Result<DockerOutput, ContainerDriverError>;
}

#[derive(Debug, Default)]
pub struct SystemDockerRunner;

#[async_trait]
impl DockerRunner for SystemDockerRunner {
    async fn run(&self, args: &[String]) -> Result<DockerOutput, ContainerDriverError> {
        let req = CommandRequest {
            cmd: "docker".to_string(),
            args: args.to_vec(),
            envs: HashMap::new(),
            timeout: DOCKER_TIMEOUT,
        };
        let res = CommandExecutor::execute(&req)
            .await
            .map_err(|e| ContainerDriverError::Engine(e.to_string()))?;
        if res.timeout {
            return Err(ContainerDriverError::Engine(
                "docker command timed out".to_string(),
            ));
        }
        Ok(DockerOutput {
            success: res.exit_code == Some(0),
            stdout: String::from_utf8_lossy(&res.stdout).into_owned(),
            stderr: String::from_utf8_lossy(&res.stderr).into_owned(),
        })
    }
}

#[derive(Deserialize)]
struct RawInspect {
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

struct InspectDetails {
    running: bool,
    labels: BTreeMap<String, String>,
}

pub struct NodeContainerDriver {
    node_id: String,
    runner: Arc<dyn DockerRunner>,
}

impl NodeContainerDriver {
    pub fn new(node_id: impl Into<String>) -> Result<Self, ContainerDriverError> {
        Self::with_runner(node_id, Arc::new(SystemDockerRunner))
    }

    pub fn with_runner(
        node_id: impl Into<String>,
        runner: Arc<dyn DockerRunner>,
    ) -> Result<Self, ContainerDriverError> {
        let node_id = node_id.into();
        let trimmed = node_id.trim();
        if !is_valid_identifier(trimmed) {
            return Err(ContainerDriverError::InvalidInput(format!(
                "invalid node_id '{node_id}'"
            )));
        }
        Ok(Self {
            node_id: trimmed.to_string(),
            runner,
        })
    }

    pub fn ownership_labels(
        &self,
        container_id: &str,
        tenant_id: Option<Uuid>,
        spec_hash: &str,
    ) -> BTreeMap<String, String> {
        let mut labels = BTreeMap::new();
        labels.insert(LABEL_MANAGED.to_string(), "true".to_string());
        labels.insert(LABEL_NODE_ID.to_string(), self.node_id.clone());
        labels.insert(LABEL_CONTAINER_ID.to_string(), container_id.to_string());
        labels.insert(LABEL_SPEC_HASH.to_string(), spec_hash.to_string());
        if let Some(tid) = tenant_id {
            labels.insert(LABEL_TENANT_ID.to_string(), tid.to_string());
        }
        labels
    }

    fn is_owned_by_node(&self, labels: &BTreeMap<String, String>, container_id: &str) -> bool {
        labels.get(LABEL_MANAGED).map(String::as_str) == Some("true")
            && labels.get(LABEL_NODE_ID).map(String::as_str) == Some(&self.node_id)
            && labels.get(LABEL_CONTAINER_ID).map(String::as_str) == Some(container_id)
    }

    fn compute_spec_hash(payload: &ContainerApplyPayload) -> String {
        let mut hash: u64 = 0xcbf29ce484222325;
        let mix = |h: &mut u64, bytes: &[u8]| {
            for &b in bytes {
                *h ^= u64::from(b);
                *h = h.wrapping_mul(0x100000001b3);
            }
            *h ^= 0xff;
            *h = h.wrapping_mul(0x100000001b3);
        };

        mix(&mut hash, payload.container_id.as_bytes());
        mix(&mut hash, payload.image.as_bytes());
        if let Some(tid) = payload.tenant_id {
            mix(&mut hash, tid.as_bytes());
        }
        if let Some(net) = &payload.network {
            mix(&mut hash, net.as_bytes());
        }
        if let Some(policy) = &payload.restart_policy {
            mix(&mut hash, policy.as_bytes());
        }
        for p in &payload.ports {
            mix(&mut hash, &p.host_port.to_le_bytes());
            mix(&mut hash, &p.container_port.to_le_bytes());
        }
        for m in &payload.mounts {
            mix(&mut hash, m.source.as_bytes());
            mix(&mut hash, m.target.as_bytes());
            mix(&mut hash, &[u8::from(m.read_only)]);
        }
        for (k, v) in &payload.env {
            mix(&mut hash, k.as_bytes());
            mix(&mut hash, v.as_bytes());
        }
        for arg in &payload.command {
            mix(&mut hash, arg.as_bytes());
        }
        format!("{hash:016x}")
    }

    async fn inspect_details(
        &self,
        container_id: &str,
    ) -> Result<Option<InspectDetails>, ContainerDriverError> {
        if !is_valid_identifier(container_id) {
            return Err(ContainerDriverError::InvalidInput(format!(
                "invalid container_id '{container_id}'"
            )));
        }
        let args = vec![
            "inspect".to_string(),
            "--format={{json .}}".to_string(),
            container_id.to_string(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            let lower = output.stderr.to_ascii_lowercase();
            if lower.contains("no such object")
                || lower.contains("no such container")
                || lower.contains("not found")
            {
                return Ok(None);
            }
            return Err(ContainerDriverError::Engine(format!(
                "docker inspect failed: {}",
                output.stderr
            )));
        }
        let record: RawInspect = serde_json::from_str(output.stdout.trim())
            .map_err(|e| ContainerDriverError::Engine(format!("invalid inspect json: {e}")))?;
        Ok(Some(InspectDetails {
            running: record.state.status.trim() == "running",
            labels: record.config.labels.unwrap_or_default(),
        }))
    }

    async fn require_owned(&self, container_id: &str) -> Result<InspectDetails, ContainerDriverError> {
        let Some(details) = self.inspect_details(container_id).await? else {
            return Err(ContainerDriverError::NotFound(container_id.to_string()));
        };
        if !self.is_owned_by_node(&details.labels, container_id) {
            return Err(ContainerDriverError::UnmanagedContainer(
                container_id.to_string(),
            ));
        }
        Ok(details)
    }

    async fn run_checked(
        &self,
        args: Vec<String>,
        op_name: &str,
    ) -> Result<(), ContainerDriverError> {
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(ContainerDriverError::Engine(format!(
                "docker {op_name} failed: {}",
                output.stderr
            )));
        }
        Ok(())
    }

    async fn create_container(
        &self,
        payload: &ContainerApplyPayload,
        spec_hash: &str,
    ) -> Result<(), ContainerDriverError> {
        if payload.image.trim().is_empty() || payload.image.chars().any(char::is_whitespace) {
            return Err(ContainerDriverError::InvalidInput(
                "invalid container image reference".to_string(),
            ));
        }
        let mut args = vec![
            "create".to_string(),
            "--name".to_string(),
            payload.container_id.clone(),
        ];
        if let Some(network) = &payload.network {
            args.push("--network".to_string());
            args.push(network.clone());
        }
        if let Some(policy) = &payload.restart_policy {
            args.push("--restart".to_string());
            args.push(policy.clone());
        }
        for port in &payload.ports {
            if port.host_port == 0 || port.container_port == 0 {
                return Err(ContainerDriverError::InvalidInput(
                    "port mapping must be non-zero".to_string(),
                ));
            }
            args.push("-p".to_string());
            args.push(format!("{}:{}", port.host_port, port.container_port));
        }
        for mount in &payload.mounts {
            let mode = if mount.read_only { "ro" } else { "rw" };
            args.push("-v".to_string());
            args.push(format!("{}:{}:{}", mount.source, mount.target, mode));
        }
        for (k, v) in &payload.env {
            args.push("-e".to_string());
            args.push(format!("{k}={v}"));
        }
        for (k, v) in self.ownership_labels(&payload.container_id, payload.tenant_id, spec_hash) {
            args.push("--label".to_string());
            args.push(format!("{k}={v}"));
        }
        args.push(payload.image.clone());
        for arg in &payload.command {
            args.push(arg.clone());
        }
        self.run_checked(args, "create").await
    }

    pub async fn apply(
        &self,
        payload: &ContainerApplyPayload,
    ) -> Result<ContainerApplyOutcome, ContainerDriverError> {
        if !is_valid_identifier(&payload.container_id) {
            return Err(ContainerDriverError::InvalidInput(format!(
                "invalid container_id '{}'",
                payload.container_id
            )));
        }
        let spec_hash = Self::compute_spec_hash(payload);

        if let Some(existing) = self.inspect_details(&payload.container_id).await? {
            if !self.is_owned_by_node(&existing.labels, &payload.container_id) {
                return Err(ContainerDriverError::NameCollision(
                    payload.container_id.clone(),
                ));
            }
            if existing.labels.get(LABEL_SPEC_HASH).map(String::as_str) == Some(spec_hash.as_str())
            {
                return if payload.desired_running {
                    if existing.running {
                        Ok(ContainerApplyOutcome::AlreadyRunning)
                    } else {
                        self.run_checked(
                            vec!["start".to_string(), payload.container_id.clone()],
                            "start",
                        )
                        .await?;
                        Ok(ContainerApplyOutcome::StartedExisting)
                    }
                } else if existing.running {
                    self.run_checked(
                        vec![
                            "stop".to_string(),
                            "-t".to_string(),
                            "10".to_string(),
                            payload.container_id.clone(),
                        ],
                        "stop",
                    )
                    .await?;
                    Ok(ContainerApplyOutcome::StoppedExisting)
                } else {
                    Ok(ContainerApplyOutcome::AlreadyStopped)
                };
            }

            self.run_checked(
                vec![
                    "rm".to_string(),
                    "-f".to_string(),
                    "-v".to_string(),
                    payload.container_id.clone(),
                ],
                "rm",
            )
            .await?;
            self.create_container(payload, &spec_hash).await?;
            if payload.desired_running {
                self.run_checked(
                    vec!["start".to_string(), payload.container_id.clone()],
                    "start",
                )
                .await?;
            }
            return Ok(ContainerApplyOutcome::Recreated);
        }

        self.create_container(payload, &spec_hash).await?;
        if payload.desired_running {
            self.run_checked(
                vec!["start".to_string(), payload.container_id.clone()],
                "start",
            )
            .await?;
        }
        Ok(ContainerApplyOutcome::Created)
    }

    pub async fn start(&self, container_id: &str) -> Result<(), ContainerDriverError> {
        let details = self.require_owned(container_id).await?;
        if details.running {
            return Ok(());
        }
        self.run_checked(
            vec!["start".to_string(), container_id.to_string()],
            "start",
        )
        .await
    }

    pub async fn stop(&self, container_id: &str) -> Result<(), ContainerDriverError> {
        let details = self.require_owned(container_id).await?;
        if !details.running {
            return Ok(());
        }
        self.run_checked(
            vec![
                "stop".to_string(),
                "-t".to_string(),
                "10".to_string(),
                container_id.to_string(),
            ],
            "stop",
        )
        .await
    }

    pub async fn restart(&self, container_id: &str) -> Result<(), ContainerDriverError> {
        let _ = self.require_owned(container_id).await?;
        self.run_checked(
            vec![
                "restart".to_string(),
                "-t".to_string(),
                "10".to_string(),
                container_id.to_string(),
            ],
            "restart",
        )
        .await
    }

    pub async fn remove(&self, container_id: &str) -> Result<bool, ContainerDriverError> {
        let Some(details) = self.inspect_details(container_id).await? else {
            return Ok(false);
        };
        if !self.is_owned_by_node(&details.labels, container_id) {
            return Err(ContainerDriverError::UnmanagedContainer(
                container_id.to_string(),
            ));
        }
        self.run_checked(
            vec![
                "rm".to_string(),
                "-f".to_string(),
                "-v".to_string(),
                container_id.to_string(),
            ],
            "rm",
        )
        .await?;
        Ok(true)
    }

    pub async fn logs(
        &self,
        payload: &ContainerLogsPayload,
        secrets: &[&str],
    ) -> Result<BoundedContainerLogs, ContainerDriverError> {
        let _ = self.require_owned(&payload.container_id).await?;
        let capped_lines = payload.max_lines.clamp(1, HARD_MAX_LOG_LINES);
        let capped_bytes = payload.max_bytes.clamp(1, HARD_MAX_LOG_BYTES);

        let args = vec![
            "logs".to_string(),
            "--tail".to_string(),
            capped_lines.to_string(),
            payload.container_id.clone(),
        ];
        let output = self.runner.run(&args).await?;
        if !output.success {
            return Err(ContainerDriverError::Engine(format!(
                "docker logs failed: {}",
                output.stderr
            )));
        }

        let mut lines = Vec::new();
        let mut total_bytes = 0usize;
        let mut truncated = false;

        for raw_line in output.stdout.lines().chain(output.stderr.lines()) {
            if lines.len() >= capped_lines {
                truncated = true;
                break;
            }
            let sanitized = sanitize_bounded(raw_line, secrets);
            if total_bytes + sanitized.len() > capped_bytes {
                let remaining = capped_bytes.saturating_sub(total_bytes);
                if remaining > 0 {
                    let mut prefix = String::new();
                    for ch in sanitized.chars() {
                        if prefix.len() + ch.len_utf8() > remaining {
                            break;
                        }
                        prefix.push(ch);
                    }
                    if !prefix.is_empty() {
                        total_bytes += prefix.len();
                        lines.push(prefix);
                    }
                }
                truncated = true;
                break;
            }
            total_bytes += sanitized.len();
            lines.push(sanitized);
        }

        Ok(BoundedContainerLogs {
            lines,
            truncated,
            total_bytes,
        })
    }

    pub async fn reconcile(
        &self,
        desired: &[ContainerApplyPayload],
    ) -> Result<ContainerReconcileReport, ContainerDriverError> {
        let mut report = ContainerReconcileReport::default();
        let desired_ids: BTreeSet<&str> =
            desired.iter().map(|d| d.container_id.as_str()).collect();

        for item in desired {
            match self.apply(item).await {
                Ok(_) => report.applied.push(item.container_id.clone()),
                Err(err) => report
                    .errors
                    .push((item.container_id.clone(), err.to_string())),
            }
        }

        let ps_args = vec![
            "ps".to_string(),
            "-a".to_string(),
            "--filter".to_string(),
            format!("label={LABEL_MANAGED}=true"),
            "--filter".to_string(),
            format!("label={LABEL_NODE_ID}={}", self.node_id),
            "--format={{.Names}}".to_string(),
        ];
        let ps_output = self.runner.run(&ps_args).await?;
        if !ps_output.success {
            return Err(ContainerDriverError::Engine(format!(
                "docker ps failed: {}",
                ps_output.stderr
            )));
        }

        for line in ps_output.stdout.lines() {
            let name = line.trim();
            if name.is_empty() || desired_ids.contains(name) {
                continue;
            }
            match self.remove(name).await {
                Ok(true) => report.removed_orphans.push(name.to_string()),
                Ok(false) => {}
                Err(ContainerDriverError::UnmanagedContainer(id)) => {
                    report.skipped_unmanaged.push(id);
                }
                Err(err) => report.errors.push((name.to_string(), err.to_string())),
            }
        }

        Ok(report)
    }
}

fn is_valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Clone)]
    struct FakeContainer {
        running: bool,
        labels: BTreeMap<String, String>,
        logs: Vec<String>,
    }

    #[derive(Default)]
    struct MockRunner {
        containers: Mutex<HashMap<String, FakeContainer>>,
        create_calls: Mutex<usize>,
    }

    impl MockRunner {
        fn insert_unmanaged(&self, id: &str, labels: BTreeMap<String, String>) {
            self.containers.lock().unwrap().insert(
                id.to_string(),
                FakeContainer {
                    running: true,
                    labels,
                    logs: vec!["sentinel-log".to_string()],
                },
            );
        }
    }

    #[async_trait]
    impl DockerRunner for MockRunner {
        async fn run(&self, args: &[String]) -> Result<DockerOutput, ContainerDriverError> {
            let mut map = self.containers.lock().unwrap();
            match args.first().map(String::as_str).unwrap_or("") {
                "inspect" => {
                    let id = args.last().cloned().unwrap_or_default();
                    if let Some(c) = map.get(&id) {
                        let status = if c.running { "running" } else { "exited" };
                        let json = serde_json::json!({
                            "State": { "Status": status },
                            "Config": { "Labels": c.labels }
                        });
                        Ok(DockerOutput {
                            success: true,
                            stdout: json.to_string(),
                            stderr: String::new(),
                        })
                    } else {
                        Ok(DockerOutput {
                            success: false,
                            stdout: String::new(),
                            stderr: format!("No such object: {id}"),
                        })
                    }
                }
                "create" => {
                    *self.create_calls.lock().unwrap() += 1;
                    let mut name = String::new();
                    let mut labels = BTreeMap::new();
                    let mut i = 1;
                    while i < args.len() {
                        match args[i].as_str() {
                            "--name" => {
                                name = args.get(i + 1).cloned().unwrap_or_default();
                                i += 2;
                            }
                            "--label" => {
                                if let Some((k, v)) =
                                    args.get(i + 1).and_then(|s| s.split_once('='))
                                {
                                    labels.insert(k.to_string(), v.to_string());
                                }
                                i += 2;
                            }
                            "--network" | "--restart" | "-p" | "-v" | "-e" => i += 2,
                            _ => i += 1,
                        }
                    }
                    map.insert(
                        name,
                        FakeContainer {
                            running: false,
                            labels,
                            logs: vec![],
                        },
                    );
                    Ok(DockerOutput {
                        success: true,
                        stdout: "id".to_string(),
                        stderr: String::new(),
                    })
                }
                "start" | "restart" => {
                    let id = args.last().cloned().unwrap_or_default();
                    if let Some(c) = map.get_mut(&id) {
                        c.running = true;
                    }
                    Ok(DockerOutput {
                        success: true,
                        stdout: id,
                        stderr: String::new(),
                    })
                }
                "stop" => {
                    let id = args.last().cloned().unwrap_or_default();
                    if let Some(c) = map.get_mut(&id) {
                        c.running = false;
                    }
                    Ok(DockerOutput {
                        success: true,
                        stdout: id,
                        stderr: String::new(),
                    })
                }
                "rm" => {
                    let id = args.last().cloned().unwrap_or_default();
                    map.remove(&id);
                    Ok(DockerOutput {
                        success: true,
                        stdout: id,
                        stderr: String::new(),
                    })
                }
                "ps" => {
                    let mut names: Vec<String> = map
                        .iter()
                        .filter(|(_, c)| {
                            c.labels.get(LABEL_MANAGED).map(String::as_str) == Some("true")
                        })
                        .map(|(k, _)| k.clone())
                        .collect();
                    names.sort();
                    Ok(DockerOutput {
                        success: true,
                        stdout: names.join("\n"),
                        stderr: String::new(),
                    })
                }
                "logs" => {
                    let id = args.last().cloned().unwrap_or_default();
                    let stdout = map
                        .get(&id)
                        .map(|c| c.logs.join("\n"))
                        .unwrap_or_default();
                    Ok(DockerOutput {
                        success: true,
                        stdout,
                        stderr: String::new(),
                    })
                }
                other => Ok(DockerOutput {
                    success: false,
                    stdout: String::new(),
                    stderr: format!("unsupported {other}"),
                }),
            }
        }
    }

    fn sample_payload(id: &str) -> ContainerApplyPayload {
        ContainerApplyPayload {
            container_id: id.to_string(),
            tenant_id: Some(Uuid::nil()),
            image: "nginx:1.25".to_string(),
            ports: vec![],
            mounts: vec![],
            network: None,
            env: BTreeMap::new(),
            command: vec![],
            restart_policy: Some("unless-stopped".to_string()),
            desired_running: true,
        }
    }

    #[tokio::test]
    async fn driver_apply_retry_and_collision_protection() {
        let runner = Arc::new(MockRunner::default());
        runner.insert_unmanaged("sentinel-c", BTreeMap::new());
        let driver = NodeContainerDriver::with_runner("node-1", runner.clone()).unwrap();

        assert_eq!(
            driver.apply(&sample_payload("sentinel-c")).await.unwrap_err(),
            ContainerDriverError::NameCollision("sentinel-c".to_string())
        );
        assert_eq!(
            driver.remove("sentinel-c").await.unwrap_err(),
            ContainerDriverError::UnmanagedContainer("sentinel-c".to_string())
        );
        assert!(runner.containers.lock().unwrap().contains_key("sentinel-c"));

        let payload = sample_payload("managed-1");
        assert_eq!(
            driver.apply(&payload).await.unwrap(),
            ContainerApplyOutcome::Created
        );
        assert_eq!(
            driver.apply(&payload).await.unwrap(),
            ContainerApplyOutcome::AlreadyRunning
        );
        driver.stop("managed-1").await.unwrap();
        assert_eq!(
            driver.apply(&payload).await.unwrap(),
            ContainerApplyOutcome::StartedExisting
        );
        assert_eq!(*runner.create_calls.lock().unwrap(), 1);
    }

    #[tokio::test]
    async fn driver_reconcile_and_bounded_logs() {
        let runner = Arc::new(MockRunner::default());
        runner.insert_unmanaged("sentinel-keep", BTreeMap::new());
        let driver = NodeContainerDriver::with_runner("node-1", runner.clone()).unwrap();

        driver.apply(&sample_payload("app-1")).await.unwrap();
        driver.apply(&sample_payload("app-orphan")).await.unwrap();
        runner.containers.lock().unwrap().get_mut("app-1").unwrap().logs = vec![
            "booting".to_string(),
            "token=secret-abc".to_string(),
            "y".repeat(200),
        ];

        let report = driver.reconcile(&[sample_payload("app-1")]).await.unwrap();
        assert_eq!(report.applied, vec!["app-1"]);
        assert_eq!(report.removed_orphans, vec!["app-orphan"]);
        assert!(runner.containers.lock().unwrap().contains_key("sentinel-keep"));
        assert!(!runner.containers.lock().unwrap().contains_key("app-orphan"));

        let logs = driver
            .logs(
                &ContainerLogsPayload {
                    container_id: "app-1".to_string(),
                    max_lines: 5,
                    max_bytes: 64,
                },
                &["secret-abc"],
            )
            .await
            .unwrap();
        assert!(logs.truncated);
        assert!(logs.total_bytes <= 64);
        assert!(!logs.lines.join("\n").contains("secret-abc"));
    }
}

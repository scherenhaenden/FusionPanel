#![forbid(unsafe_code)]

use async_trait::async_trait;
use containers::docker::adapter::{
    ApplyOutcome, DesiredContainer, DockerAdapterError, NodeDockerAdapter, RemoveOutcome,
    LABEL_CONTAINER_ID, LABEL_MANAGED, LABEL_NODE_ID, LABEL_SPEC_HASH, LABEL_TENANT_ID,
};
use containers::docker::engine::{DockerCliEngine, DockerCommandOutput, DockerCommandRunner};
use containers::domain::{ContainerId, ContainerSpec, ContainerStatus, DomainError, ImageRef};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
struct FakeContainer {
    status: ContainerStatus,
    labels: BTreeMap<String, String>,
    logs: Vec<String>,
}

#[derive(Default)]
struct FakeDockerState {
    containers: HashMap<String, FakeContainer>,
    commands: Vec<Vec<String>>,
    fail_next_create: Option<String>,
}

#[derive(Clone, Default)]
struct MockDockerRunner {
    state: Arc<Mutex<FakeDockerState>>,
}

impl MockDockerRunner {
    fn insert_unmanaged(&self, name: &str, status: ContainerStatus, labels: BTreeMap<String, String>) {
        self.state.lock().unwrap().containers.insert(
            name.to_string(),
            FakeContainer {
                status,
                labels,
                logs: vec!["unmanaged log".to_string()],
            },
        );
    }

    fn set_logs(&self, name: &str, logs: Vec<String>) {
        if let Some(c) = self.state.lock().unwrap().containers.get_mut(name) {
            c.logs = logs;
        }
    }

    fn commands(&self) -> Vec<Vec<String>> {
        self.state.lock().unwrap().commands.clone()
    }

    fn exists(&self, name: &str) -> bool {
        self.state.lock().unwrap().containers.contains_key(name)
    }
}

fn status_str(status: ContainerStatus) -> &'static str {
    match status {
        ContainerStatus::Created => "created",
        ContainerStatus::Running => "running",
        ContainerStatus::Stopped | ContainerStatus::Exited => "exited",
        ContainerStatus::Removed => "",
    }
}

#[async_trait]
impl DockerCommandRunner for MockDockerRunner {
    async fn run(&self, args: &[String]) -> Result<DockerCommandOutput, DomainError> {
        let mut state = self.state.lock().unwrap();
        state.commands.push(args.to_vec());

        let sub = args.first().map(String::as_str).unwrap_or("");
        match sub {
            "inspect" => {
                let id = args.last().cloned().unwrap_or_default();
                let format_arg = args.get(1).map(String::as_str).unwrap_or("");
                if let Some(c) = state.containers.get(&id) {
                    if format_arg.contains("json") {
                        let json = serde_json::json!({
                            "State": { "Status": status_str(c.status) },
                            "Config": { "Labels": c.labels }
                        });
                        Ok(DockerCommandOutput {
                            success: true,
                            stdout: json.to_string(),
                            stderr: String::new(),
                        })
                    } else {
                        Ok(DockerCommandOutput {
                            success: true,
                            stdout: status_str(c.status).to_string(),
                            stderr: String::new(),
                        })
                    }
                } else {
                    Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("Error: No such object: {id}"),
                    })
                }
            }
            "create" => {
                if let Some(err) = state.fail_next_create.take() {
                    return Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: err,
                    });
                }
                let mut name = String::new();
                let mut labels = BTreeMap::new();
                let mut i = 1;
                while i < args.len() {
                    match args[i].as_str() {
                        "--name" => {
                            if let Some(n) = args.get(i + 1) {
                                name = n.clone();
                            }
                            i += 2;
                        }
                        "--label" => {
                            if let Some(kv) = args.get(i + 1) {
                                if let Some((k, v)) = kv.split_once('=') {
                                    labels.insert(k.to_string(), v.to_string());
                                }
                            }
                            i += 2;
                        }
                        "--network" | "--restart" | "-p" | "-v" | "-e" => {
                            i += 2;
                        }
                        _ => i += 1,
                    }
                }
                if state.containers.contains_key(&name) {
                    return Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("Conflict. The container name \"/{name}\" is already in use"),
                    });
                }
                state.containers.insert(
                    name,
                    FakeContainer {
                        status: ContainerStatus::Created,
                        labels,
                        logs: Vec::new(),
                    },
                );
                Ok(DockerCommandOutput {
                    success: true,
                    stdout: "mock-id".to_string(),
                    stderr: String::new(),
                })
            }
            "start" => {
                let id = args.last().cloned().unwrap_or_default();
                if let Some(c) = state.containers.get_mut(&id) {
                    c.status = ContainerStatus::Running;
                    Ok(DockerCommandOutput {
                        success: true,
                        stdout: id,
                        stderr: String::new(),
                    })
                } else {
                    Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("No such container: {id}"),
                    })
                }
            }
            "stop" => {
                let id = args.last().cloned().unwrap_or_default();
                if let Some(c) = state.containers.get_mut(&id) {
                    c.status = ContainerStatus::Exited;
                    Ok(DockerCommandOutput {
                        success: true,
                        stdout: id,
                        stderr: String::new(),
                    })
                } else {
                    Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("No such container: {id}"),
                    })
                }
            }
            "restart" => {
                let id = args.last().cloned().unwrap_or_default();
                if let Some(c) = state.containers.get_mut(&id) {
                    c.status = ContainerStatus::Running;
                    Ok(DockerCommandOutput {
                        success: true,
                        stdout: id,
                        stderr: String::new(),
                    })
                } else {
                    Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("No such container: {id}"),
                    })
                }
            }
            "rm" => {
                let id = args.last().cloned().unwrap_or_default();
                if state.containers.remove(&id).is_some() {
                    Ok(DockerCommandOutput {
                        success: true,
                        stdout: id,
                        stderr: String::new(),
                    })
                } else {
                    Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("No such container: {id}"),
                    })
                }
            }
            "ps" => {
                let mut filters = Vec::new();
                let mut i = 1;
                while i < args.len() {
                    if args[i] == "--filter" {
                        if let Some(f) = args.get(i + 1) {
                            if let Some(kv) = f.strip_prefix("label=") {
                                if let Some((k, v)) = kv.split_once('=') {
                                    filters.push((k.to_string(), v.to_string()));
                                }
                            }
                        }
                        i += 2;
                    } else {
                        i += 1;
                    }
                }
                let mut matched = Vec::new();
                for (name, c) in &state.containers {
                    if filters
                        .iter()
                        .all(|(k, v)| c.labels.get(k).map(String::as_str) == Some(v.as_str()))
                    {
                        matched.push(name.clone());
                    }
                }
                matched.sort();
                Ok(DockerCommandOutput {
                    success: true,
                    stdout: matched.join("\n"),
                    stderr: String::new(),
                })
            }
            "logs" => {
                let tail: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(100);
                let id = args.last().cloned().unwrap_or_default();
                if let Some(c) = state.containers.get(&id) {
                    let start = c.logs.len().saturating_sub(tail);
                    Ok(DockerCommandOutput {
                        success: true,
                        stdout: c.logs[start..].join("\n"),
                        stderr: String::new(),
                    })
                } else {
                    Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("No such container: {id}"),
                    })
                }
            }
            _ => Ok(DockerCommandOutput {
                success: false,
                stdout: String::new(),
                stderr: format!("unsupported command: {sub}"),
            }),
        }
    }
}

fn sample_spec(id: &str, image: &str) -> ContainerSpec {
    ContainerSpec::new(
        ContainerId::new(id.to_string()).unwrap(),
        ImageRef::new(image.to_string()).unwrap(),
        vec![],
        vec![],
    )
}

#[tokio::test]
async fn apply_attaches_ownership_labels_and_retries_without_duplicates() {
    let runner = Arc::new(MockDockerRunner::default());
    let engine = Arc::new(DockerCliEngine::with_runner(runner.clone()));
    let adapter = NodeDockerAdapter::new("node-1", engine).unwrap();

    let spec = sample_spec("tenant1_web", "nginx:1.25");
    let first = adapter.apply(&spec, Some("tenant-1"), true).await.unwrap();
    assert_eq!(first, ApplyOutcome::Created);

    // Verify ownership labels stored on container
    {
        let state = runner.state.lock().unwrap();
        let c = state.containers.get("tenant1_web").unwrap();
        assert_eq!(c.status, ContainerStatus::Running);
        assert_eq!(c.labels.get(LABEL_MANAGED).map(String::as_str), Some("true"));
        assert_eq!(c.labels.get(LABEL_NODE_ID).map(String::as_str), Some("node-1"));
        assert_eq!(
            c.labels.get(LABEL_CONTAINER_ID).map(String::as_str),
            Some("tenant1_web")
        );
        assert_eq!(
            c.labels.get(LABEL_TENANT_ID).map(String::as_str),
            Some("tenant-1")
        );
        assert!(c.labels.contains_key(LABEL_SPEC_HASH));
    }

    // Retry while running -> AlreadyRunning, no create or duplicate
    let retry_running = adapter.apply(&spec, Some("tenant-1"), true).await.unwrap();
    assert_eq!(retry_running, ApplyOutcome::AlreadyRunning);

    // Stop and retry apply(desired_running=true) -> starts existing without create
    adapter.stop(&spec.id).await.unwrap();
    let retry_stopped = adapter.apply(&spec, Some("tenant-1"), true).await.unwrap();
    assert_eq!(retry_stopped, ApplyOutcome::StartedExisting);

    let create_count = runner
        .commands()
        .iter()
        .filter(|cmd| cmd.first().map(String::as_str) == Some("create"))
        .count();
    assert_eq!(create_count, 1, "retry must never create duplicate containers");
}

#[tokio::test]
async fn collision_and_lifecycle_refuse_to_touch_or_delete_unmanaged_sentinel() {
    let runner = Arc::new(MockDockerRunner::default());
    // Unmanaged sentinel container
    runner.insert_unmanaged("sentinel-db", ContainerStatus::Running, BTreeMap::new());
    // Foreign-node container
    let mut foreign_labels = BTreeMap::new();
    foreign_labels.insert(LABEL_MANAGED.to_string(), "true".to_string());
    foreign_labels.insert(LABEL_NODE_ID.to_string(), "node-other".to_string());
    foreign_labels.insert(LABEL_CONTAINER_ID.to_string(), "foreign-app".to_string());
    runner.insert_unmanaged("foreign-app", ContainerStatus::Running, foreign_labels);

    let engine = Arc::new(DockerCliEngine::with_runner(runner.clone()));
    let adapter = NodeDockerAdapter::new("node-1", engine).unwrap();

    let sentinel_spec = sample_spec("sentinel-db", "postgres:16");
    let err = adapter
        .apply(&sentinel_spec, Some("tenant-1"), true)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        DockerAdapterError::NameCollision("sentinel-db".to_string())
    );

    let sentinel_id = ContainerId::new("sentinel-db".to_string()).unwrap();
    assert_eq!(
        adapter.start(&sentinel_id).await.unwrap_err(),
        DockerAdapterError::UnmanagedContainer("sentinel-db".to_string())
    );
    assert_eq!(
        adapter.stop(&sentinel_id).await.unwrap_err(),
        DockerAdapterError::UnmanagedContainer("sentinel-db".to_string())
    );
    assert_eq!(
        adapter.restart(&sentinel_id).await.unwrap_err(),
        DockerAdapterError::UnmanagedContainer("sentinel-db".to_string())
    );
    assert_eq!(
        adapter.remove(&sentinel_id).await.unwrap_err(),
        DockerAdapterError::UnmanagedContainer("sentinel-db".to_string())
    );
    assert_eq!(
        adapter.logs(&sentinel_id, 50, 4096).await.unwrap_err(),
        DockerAdapterError::UnmanagedContainer("sentinel-db".to_string())
    );

    let foreign_id = ContainerId::new("foreign-app".to_string()).unwrap();
    assert_eq!(
        adapter.remove(&foreign_id).await.unwrap_err(),
        DockerAdapterError::UnmanagedContainer("foreign-app".to_string())
    );

    assert!(runner.exists("sentinel-db"));
    assert!(runner.exists("foreign-app"));
}

#[tokio::test]
async fn restart_reconciliation_syncs_desired_and_removes_only_managed_orphans() {
    let runner = Arc::new(MockDockerRunner::default());
    // Sentinel unmanaged container must survive reconciliation untouched
    runner.insert_unmanaged("sentinel-proxy", ContainerStatus::Running, BTreeMap::new());
    // Foreign node container must survive reconciliation untouched
    let mut foreign_labels = BTreeMap::new();
    foreign_labels.insert(LABEL_MANAGED.to_string(), "true".to_string());
    foreign_labels.insert(LABEL_NODE_ID.to_string(), "node-2".to_string());
    foreign_labels.insert(LABEL_CONTAINER_ID.to_string(), "other-node-app".to_string());
    runner.insert_unmanaged("other-node-app", ContainerStatus::Running, foreign_labels);

    let engine = Arc::new(DockerCliEngine::with_runner(runner.clone()));
    let adapter = NodeDockerAdapter::new("node-1", engine).unwrap();

    // Create two managed containers on node-1
    let app_keep = sample_spec("app-keep", "alpine:3.19");
    let app_orphan = sample_spec("app-orphan", "alpine:3.19");
    adapter.apply(&app_keep, Some("t1"), true).await.unwrap();
    adapter.apply(&app_orphan, Some("t1"), true).await.unwrap();
    // Simulate app-keep stopped after host reboot
    adapter.stop(&app_keep.id).await.unwrap();

    let desired = vec![
        DesiredContainer {
            spec: app_keep.clone(),
            tenant_id: Some("t1".to_string()),
            desired_running: true,
        },
        DesiredContainer {
            spec: sample_spec("app-new", "alpine:3.19"),
            tenant_id: Some("t1".to_string()),
            desired_running: true,
        },
    ];

    let report = adapter.reconcile(&desired).await.unwrap();
    assert_eq!(report.applied, vec!["app-keep", "app-new"]);
    assert_eq!(report.removed_orphans, vec!["app-orphan"]);
    assert!(report.errors.is_empty());

    assert!(!runner.exists("app-orphan"));
    assert!(runner.exists("app-keep"));
    assert!(runner.exists("app-new"));
    assert!(runner.exists("sentinel-proxy"));
    assert!(runner.exists("other-node-app"));
    assert_eq!(
        adapter.inspect_owned(&app_keep.id).await.unwrap(),
        ContainerStatus::Running
    );
}

#[tokio::test]
async fn bounded_logs_redacts_secrets_and_caps_bytes_and_lines() {
    let runner = Arc::new(MockDockerRunner::default());
    let engine = Arc::new(DockerCliEngine::with_runner(runner.clone()));
    let adapter = NodeDockerAdapter::new("node-1", engine).unwrap();

    let spec = sample_spec("app-logs", "alpine:3.19");
    adapter.apply(&spec, Some("t1"), true).await.unwrap();
    runner.set_logs(
        "app-logs",
        vec![
            "starting service".to_string(),
            "auth token=super-secret-value-123".to_string(),
            "password: another-secret".to_string(),
            "x".repeat(200),
        ],
    );

    let logs = adapter.logs(&spec.id, 3, 120).await.unwrap();
    assert!(logs.truncated);
    assert!(logs.total_bytes <= 120);
    let joined = logs.lines.join("\n");
    assert!(!joined.contains("super-secret-value-123"));
    assert!(!joined.contains("another-secret"));
    assert!(joined.contains("***REDACTED***"));

    // Missing container returns NotFound
    let missing_id = ContainerId::new("does-not-exist".to_string()).unwrap();
    assert_eq!(
        adapter.remove(&missing_id).await.unwrap(),
        RemoveOutcome::NotFound
    );
    assert_eq!(
        adapter.start(&missing_id).await.unwrap_err(),
        DockerAdapterError::NotFound("does-not-exist".to_string())
    );
}

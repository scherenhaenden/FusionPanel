#![forbid(unsafe_code)]

use async_trait::async_trait;
use containers::docker::adapter::{
    ApplyOutcome, DesiredContainer, NodeDockerAdapter, RemoveOutcome, LABEL_MANAGED,
};
use containers::docker::engine::{DockerCliEngine, DockerCommandOutput, DockerCommandRunner};
use containers::domain::{ContainerId, ContainerSpec, ContainerStatus, DomainError, ImageRef, PortMapping};
use integration_containers::ports::PortAllocator;
use integration_containers::store::{ContainerStore, SqliteContainerStore};
use integration_containers::worker::{ContainerWorkerError, ContainerWorkerService};
use rusqlite::Connection;
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug)]
struct FakeContainer {
    status: ContainerStatus,
    labels: BTreeMap<String, String>,
    logs: Vec<String>,
}

#[derive(Default)]
struct FakeRunnerState {
    containers: HashMap<String, FakeContainer>,
    create_calls: usize,
}

#[derive(Clone, Default)]
struct FakeRunner {
    state: Arc<Mutex<FakeRunnerState>>,
}

impl FakeRunner {
    fn insert_unmanaged(&self, id: &str) {
        self.state.lock().unwrap().containers.insert(
            id.to_string(),
            FakeContainer {
                status: ContainerStatus::Running,
                labels: BTreeMap::new(),
                logs: vec!["sentinel".to_string()],
            },
        );
    }

    fn set_logs(&self, id: &str, logs: Vec<String>) {
        if let Some(c) = self.state.lock().unwrap().containers.get_mut(id) {
            c.logs = logs;
        }
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
impl DockerCommandRunner for FakeRunner {
    async fn run(&self, args: &[String]) -> Result<DockerCommandOutput, DomainError> {
        let mut state = self.state.lock().unwrap();
        let sub = args.first().map(String::as_str).unwrap_or("");
        match sub {
            "inspect" => {
                let id = args.last().cloned().unwrap_or_default();
                let fmt = args.get(1).map(String::as_str).unwrap_or("");
                if let Some(c) = state.containers.get(&id) {
                    let stdout = if fmt.contains("json") {
                        serde_json::json!({
                            "State": { "Status": status_str(c.status) },
                            "Config": { "Labels": c.labels }
                        })
                        .to_string()
                    } else {
                        status_str(c.status).to_string()
                    };
                    Ok(DockerCommandOutput {
                        success: true,
                        stdout,
                        stderr: String::new(),
                    })
                } else {
                    Ok(DockerCommandOutput {
                        success: false,
                        stdout: String::new(),
                        stderr: format!("No such object: {id}"),
                    })
                }
            }
            "create" => {
                state.create_calls += 1;
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
                        "--network" | "--restart" | "-p" | "-v" | "-e" => i += 2,
                        _ => i += 1,
                    }
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
                    stdout: "ok".to_string(),
                    stderr: String::new(),
                })
            }
            "start" => {
                let id = args.last().cloned().unwrap_or_default();
                if let Some(c) = state.containers.get_mut(&id) {
                    c.status = ContainerStatus::Running;
                }
                Ok(DockerCommandOutput {
                    success: true,
                    stdout: id,
                    stderr: String::new(),
                })
            }
            "stop" => {
                let id = args.last().cloned().unwrap_or_default();
                if let Some(c) = state.containers.get_mut(&id) {
                    c.status = ContainerStatus::Exited;
                }
                Ok(DockerCommandOutput {
                    success: true,
                    stdout: id,
                    stderr: String::new(),
                })
            }
            "restart" => {
                let id = args.last().cloned().unwrap_or_default();
                if let Some(c) = state.containers.get_mut(&id) {
                    c.status = ContainerStatus::Running;
                }
                Ok(DockerCommandOutput {
                    success: true,
                    stdout: id,
                    stderr: String::new(),
                })
            }
            "rm" => {
                let id = args.last().cloned().unwrap_or_default();
                state.containers.remove(&id);
                Ok(DockerCommandOutput {
                    success: true,
                    stdout: id,
                    stderr: String::new(),
                })
            }
            "ps" => {
                let mut names: Vec<String> = state
                    .containers
                    .iter()
                    .filter(|(_, c)| c.labels.get(LABEL_MANAGED).map(String::as_str) == Some("true"))
                    .map(|(n, _)| n.clone())
                    .collect();
                names.sort();
                Ok(DockerCommandOutput {
                    success: true,
                    stdout: names.join("\n"),
                    stderr: String::new(),
                })
            }
            "logs" => {
                let id = args.last().cloned().unwrap_or_default();
                let stdout = state
                    .containers
                    .get(&id)
                    .map(|c| c.logs.join("\n"))
                    .unwrap_or_default();
                Ok(DockerCommandOutput {
                    success: true,
                    stdout,
                    stderr: String::new(),
                })
            }
            _ => Ok(DockerCommandOutput {
                success: false,
                stdout: String::new(),
                stderr: "unsupported".to_string(),
            }),
        }
    }
}

fn setup_worker() -> (ContainerWorkerService, Arc<FakeRunner>, Arc<SqliteContainerStore>, Arc<PortAllocator>) {
    let runner = Arc::new(FakeRunner::default());
    let engine = Arc::new(DockerCliEngine::with_runner(runner.clone()));
    let adapter = Arc::new(NodeDockerAdapter::new("node-a", engine).unwrap());
    let store = Arc::new(SqliteContainerStore::new(Connection::open_in_memory().unwrap()).unwrap());
    let ports = Arc::new(PortAllocator::new(25000, 25100));
    let worker = ContainerWorkerService::new(adapter, store.clone(), ports.clone());
    (worker, runner, store, ports)
}

#[tokio::test]
async fn worker_lifecycle_retry_and_port_rollback_on_collision() {
    let (worker, runner, store, ports) = setup_worker();
    runner.insert_unmanaged("sentinel-app");

    let host_port = ports.allocate().unwrap();
    let collision_spec = ContainerSpec::new(
        ContainerId::new("sentinel-app".to_string()).unwrap(),
        ImageRef::new("nginx:latest".to_string()).unwrap(),
        vec![PortMapping::new(host_port, 80).unwrap()],
        vec![],
    );

    let err = worker
        .apply_container("tenant-a", &collision_spec, true)
        .await
        .unwrap_err();
    assert_eq!(
        err,
        ContainerWorkerError::NameCollision("sentinel-app".to_string())
    );
    // Store record rolled back and port released
    assert_eq!(store.get_container_tenant("sentinel-app").unwrap(), None);
    assert!(ports.allocate_specific(host_port).is_ok());
    assert!(runner.state.lock().unwrap().containers.contains_key("sentinel-app"));

    // Managed container apply + retry without duplicate + logs + remove
    let app_spec = ContainerSpec::new(
        ContainerId::new("tenant-a-web".to_string()).unwrap(),
        ImageRef::new("nginx:latest".to_string()).unwrap(),
        vec![PortMapping::new(host_port, 80).unwrap()],
        vec![],
    );
    assert_eq!(
        worker
            .apply_container("tenant-a", &app_spec, true)
            .await
            .unwrap(),
        ApplyOutcome::Created
    );
    assert_eq!(
        worker
            .apply_container("tenant-a", &app_spec, true)
            .await
            .unwrap(),
        ApplyOutcome::AlreadyRunning
    );
    assert_eq!(runner.state.lock().unwrap().create_calls, 1);

    // Tenant B cannot access Tenant A's container
    assert!(matches!(
        worker
            .stop_container("tenant-b", &app_spec.id)
            .await
            .unwrap_err(),
        ContainerWorkerError::AccessDenied(_)
    ));

    runner.set_logs(
        "tenant-a-web",
        vec!["ok".to_string(), "secret=my-db-pass".to_string()],
    );
    let logs = worker
        .fetch_logs("tenant-a", &app_spec.id, 50, 2048)
        .await
        .unwrap();
    assert!(!logs.lines.join("\n").contains("my-db-pass"));
    assert!(logs.lines.join("\n").contains("***REDACTED***"));

    assert_eq!(
        worker
            .remove_container("tenant-a", &app_spec.id)
            .await
            .unwrap(),
        RemoveOutcome::Removed
    );
    assert_eq!(store.get_container_tenant("tenant-a-web").unwrap(), None);
}

#[tokio::test]
async fn worker_reconcile_preserves_sentinel_containers() {
    let (worker, runner, _, _) = setup_worker();
    runner.insert_unmanaged("sentinel-worker");

    let desired = vec![DesiredContainer {
        spec: ContainerSpec::new(
            ContainerId::new("managed-api".to_string()).unwrap(),
            ImageRef::new("alpine:3.19".to_string()).unwrap(),
            vec![],
            vec![],
        ),
        tenant_id: Some("tenant-a".to_string()),
        desired_running: true,
    }];

    let report = worker.reconcile(&desired).await.unwrap();
    assert_eq!(report.applied, vec!["managed-api"]);
    assert!(report.removed_orphans.is_empty());
    assert!(runner.state.lock().unwrap().containers.contains_key("sentinel-worker"));
    assert!(runner.state.lock().unwrap().containers.contains_key("managed-api"));
}

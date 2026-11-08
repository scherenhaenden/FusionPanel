#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;
use tokio::fs;

use fusionpanel_agent_protocol::{Operation, SystemInfoPayload};
use fusionpanel_jobs::{JobState, JobStore};
use integration_jobs::agent_subprocess::SubprocessAgentClient;
use integration_jobs::{OperationService, Worker};

#[tokio::test]
#[cfg(unix)]
async fn test_real_worker_subprocess() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("jobs.db");

    let script_path = temp_dir.path().join("dummy_agent.sh");
    let script_content = r#"#!/bin/bash
read payload
request_id=$(echo "$payload" | grep -o '"request_id":"[^"]*"' | cut -d'"' -f4)
cat <<JSONEOF
{"version":1,"request_id":"$request_id","result":{"status":"ok"}}
JSONEOF
"#;

    fs::write(&script_path, script_content).await.unwrap();

    let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&script_path, perms).unwrap();

    let store = Arc::new(Mutex::new(JobStore::new(&db_path).unwrap()));
    let service = OperationService::new(store.clone());

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    let client = Arc::new(SubprocessAgentClient {
        executable_path: script_path.to_str().unwrap().to_string(),
    });

    let worker = Worker::new(
        "worker-1".to_string(),
        store.clone(),
        client,
        Duration::from_secs(5),
    );

    let processed = worker.run_once().await.unwrap();
    assert!(processed);

    let job = service.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Succeeded);
}

#[tokio::test]
#[cfg(unix)]
async fn test_worker_kill_restart_reconciliation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("jobs_kill.db");

    let script_path = temp_dir.path().join("sleep_agent.sh");
    let script_content = r#"#!/bin/bash
sleep 10
"#;
    fs::write(&script_path, script_content).await.unwrap();

    let mut perms = std::fs::metadata(&script_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut perms, 0o755);
    std::fs::set_permissions(&script_path, perms).unwrap();

    let store = Arc::new(Mutex::new(JobStore::new(&db_path).unwrap()));
    let service = OperationService::new(store.clone());

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    // Mimic the worker claiming the job and then getting killed.
    // We claim the job explicitly in SQLite.
    {
        let mut store_lock = store.lock().unwrap();
        let claimed = store_lock.claim("killed-worker").unwrap();
        assert!(claimed.is_some());
    }

    // Simulate process kill and restart by dropping the in-memory handles.
    drop(service);
    drop(store);

    // Restart worker node
    let store2 = Arc::new(Mutex::new(JobStore::new(&db_path).unwrap()));
    let service2 = OperationService::new(store2.clone());

    // Job should STILL be running (it was interrupted)
    let job = service2.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Running);

    // Another worker should NOT automatically retry it.
    let client = Arc::new(SubprocessAgentClient {
        executable_path: script_path.to_str().unwrap().to_string(),
    });
    let worker2 = Worker::new(
        "worker-restarted".to_string(),
        store2.clone(),
        client,
        Duration::from_secs(5),
    );
    let processed = worker2.run_once().await.unwrap();
    assert!(!processed); // Queue is empty, no blind replay!

    // Verify we can find it in the reconciliation API
    let running = service2.get_running_operations().unwrap();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].id, id);

    // Explicit manual reconciliation: operator cancels the job
    service2.cancel_operation(&id).unwrap();
    let job = service2.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
}

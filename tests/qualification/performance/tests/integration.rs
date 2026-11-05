use performance_tests::runner::run_load_test;
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[tokio::test]
async fn test_actual_api_load() {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(2))
        .build()
        .unwrap();
    let health_url = "http://127.0.0.1:8080/v1/health";

    let probe = client.get(health_url).send().await.unwrap_or_else(|error| {
        panic!("BLOCKED: disposable FusionPanel API is unavailable at {health_url}: {error}")
    });
    assert!(
        probe.status().is_success(),
        "FusionPanel API health endpoint returned {}",
        probe.status()
    );

    let result = run_load_test(10, 100, move || {
        let client = client.clone();
        async move {
            match client.get(health_url).send().await {
                Ok(response) if response.status().is_success() => Ok(()),
                _ => Err(()),
            }
        }
    })
    .await;

    let total = result.success_count + result.error_count;
    let error_rate = result.error_count as f64 / total as f64;
    println!(
        "API health workload: requests={total}, successes={}, errors={}, error_rate={error_rate:.4}, duration={:?}, p50={:?}, p95={:?}, p99={:?}",
        result.success_count,
        result.error_count,
        result.duration,
        result.p50(),
        result.p95(),
        result.p99()
    );

    assert_eq!(total, 1000);
    assert_eq!(
        result.error_count, 0,
        "health workload returned failed requests"
    );
    assert!(result.p50().is_some());
    assert!(result.p95().is_some());
    assert!(result.p99().is_some());
}

#[tokio::test]
#[cfg(unix)]
async fn test_worker_failure_is_persisted_by_real_runtime() {
    use fusionpanel_jobs::{JobState, JobStore};
    use integration_jobs::{agent_subprocess::SubprocessAgentClient, Worker};

    const REQUEST_ID: &str = "00000000-0000-0000-0000-000000000001";

    let temporary = tempfile::tempdir().unwrap();
    let database_path = temporary.path().join("jobs.sqlite");
    let agent_path = temporary.path().join("controlled-agent.sh");
    let response = serde_json::json!({
        "version": 1,
        "request_id": REQUEST_ID,
        "error": {
            "code": "injected",
            "message": "controlled failure"
        }
    });
    let script = format!("#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{}'\n", response);
    std::fs::write(&agent_path, script).unwrap();
    let mut permissions = std::fs::metadata(&agent_path).unwrap().permissions();
    std::os::unix::fs::PermissionsExt::set_mode(&mut permissions, 0o700);
    std::fs::set_permissions(&agent_path, permissions).unwrap();

    let request = serde_json::json!({
        "version": 1,
        "request_id": REQUEST_ID,
        "operation": "system.info",
        "payload": {}
    });
    let store = JobStore::new(&database_path).unwrap();
    let job_id = store.create("agent_operation", &request).unwrap();
    let shared_store = Arc::new(Mutex::new(store));
    let agent = Arc::new(SubprocessAgentClient {
        executable_path: agent_path.to_string_lossy().into_owned(),
    });
    let worker = Worker::new(
        "performance-fixture".to_string(),
        Arc::clone(&shared_store),
        agent,
        Duration::from_secs(2),
    );

    let started = Instant::now();
    assert!(worker.run_once().await.unwrap());
    let failure_processing_time = started.elapsed();

    drop(worker);
    drop(shared_store);

    let reopened = JobStore::new(&database_path).unwrap();
    let job = reopened.get(&job_id).unwrap();
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.worker_id.as_deref(), Some("performance-fixture"));
    assert_eq!(
        job.error_text.as_deref(),
        Some("Agent error: controlled failure")
    );
    println!(
        "Persisted controlled agent failure in {:?}; worker restart recovery is not exercised.",
        failure_processing_time
    );
}

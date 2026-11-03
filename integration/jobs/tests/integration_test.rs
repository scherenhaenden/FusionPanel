#![forbid(unsafe_code)]

use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use serde_json::json;
use tokio::time::sleep;

use fusionpanel_agent_protocol::{Operation, Request, Response, ResponseResult, SystemInfoPayload};
use fusionpanel_jobs::{JobState, JobStore};
use integration_jobs::{AgentClient, OperationService, Worker};

struct MockAgentClient {
    delay: Duration,
    should_fail: bool,
}

#[async_trait]
impl AgentClient for MockAgentClient {
    async fn execute(&self, request: Request) -> Result<Response, String> {
        if self.delay > Duration::from_millis(0) {
            sleep(self.delay).await;
        }

        if self.should_fail {
            Ok(Response {
                version: 1,
                request_id: request.request_id,
                result: ResponseResult::Error(fusionpanel_agent_protocol::ErrorObject {
                    code: "test_error".to_string(),
                    message: "Intentional failure".to_string(),
                }),
            })
        } else {
            Ok(Response {
                version: 1,
                request_id: request.request_id,
                result: ResponseResult::Result(json!({"status": "ok"})),
            })
        }
    }
}

#[tokio::test]
async fn test_successful_operation() {
    let store = Arc::new(Mutex::new(JobStore::in_memory().unwrap()));
    let service = OperationService::new(store.clone());

    let client = Arc::new(MockAgentClient {
        delay: Duration::from_millis(0),
        should_fail: false,
    });

    let worker = Worker::new(
        "worker-1".to_string(),
        store.clone(),
        client,
        Duration::from_secs(5),
    );

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    let job = service.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Queued);

    let processed = worker.run_once().await.unwrap();
    assert!(processed);

    let job = service.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Succeeded);
}

#[tokio::test]
async fn test_failed_operation() {
    let store = Arc::new(Mutex::new(JobStore::in_memory().unwrap()));
    let service = OperationService::new(store.clone());

    let client = Arc::new(MockAgentClient {
        delay: Duration::from_millis(0),
        should_fail: true,
    });

    let worker = Worker::new(
        "worker-1".to_string(),
        store.clone(),
        client,
        Duration::from_secs(5),
    );

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    let processed = worker.run_once().await.unwrap();
    assert!(processed);

    let job = service.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.error_text.unwrap(), "Agent error: Intentional failure");
}

#[tokio::test]
async fn test_execution_timeout() {
    let store = Arc::new(Mutex::new(JobStore::in_memory().unwrap()));
    let service = OperationService::new(store.clone());

    let client = Arc::new(MockAgentClient {
        delay: Duration::from_millis(100),
        should_fail: false,
    });

    let worker = Worker::new(
        "worker-1".to_string(),
        store.clone(),
        client,
        Duration::from_millis(10), // shorter than delay
    );

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    let processed = worker.run_once().await.unwrap();
    assert!(processed);

    let job = service.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Failed);
    assert_eq!(job.error_text.unwrap(), "Execution timed out");
}

#[tokio::test]
async fn test_cancellation() {
    let store = Arc::new(Mutex::new(JobStore::in_memory().unwrap()));
    let service = OperationService::new(store.clone());

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    service.cancel_operation(&id).unwrap();

    let job = service.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
}

#[tokio::test]
async fn test_no_blind_replay_and_reconciliation() {
    let temp_dir = tempfile::tempdir().unwrap();
    let db_path = temp_dir.path().join("jobs.db");

    let store = Arc::new(Mutex::new(JobStore::new(&db_path).unwrap()));
    let service = OperationService::new(store.clone());

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    // Simulate worker claiming the job and then "crashing" before finishing
    {
        let mut store_lock = store.lock().unwrap();
        let claimed = store_lock.claim("worker-1").unwrap();
        assert!(claimed.is_some());
    }

    // Now reload the store from disk
    let store2 = Arc::new(Mutex::new(JobStore::new(&db_path).unwrap()));
    let service2 = OperationService::new(store2.clone());

    let job = service2.get_operation(&id).unwrap();
    // It should still be Running
    assert_eq!(job.state, JobState::Running);

    // It should not be claimed again by run_once
    let client = Arc::new(MockAgentClient {
        delay: Duration::from_millis(0),
        should_fail: false,
    });
    let worker = Worker::new(
        "worker-2".to_string(),
        store2.clone(),
        client,
        Duration::from_secs(5),
    );

    let processed = worker.run_once().await.unwrap();
    assert!(!processed); // Nothing queued to claim

    // Should appear in reconciliation list
    let running = service2.get_running_operations().unwrap();
    assert_eq!(running.len(), 1);
    assert_eq!(running[0].id, id);
    assert_eq!(running[0].worker_id.as_deref(), Some("worker-1"));

    // Operator manually transitions it if desired (or cancels it)
    service2.cancel_operation(&id).unwrap();
    let job = service2.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Cancelled);
}

#[tokio::test]
async fn test_competing_claims() {
    let store = Arc::new(Mutex::new(JobStore::in_memory().unwrap()));
    let service = OperationService::new(store.clone());

    let client1 = Arc::new(MockAgentClient {
        delay: Duration::from_millis(50),
        should_fail: false,
    });
    let client2 = Arc::new(MockAgentClient {
        delay: Duration::from_millis(50),
        should_fail: false,
    });

    let worker1 = Worker::new(
        "worker-1".to_string(),
        store.clone(),
        client1,
        Duration::from_secs(5),
    );
    let worker2 = Worker::new(
        "worker-2".to_string(),
        store.clone(),
        client2,
        Duration::from_secs(5),
    );

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    let job = service.get_operation(&id).unwrap();
    assert_eq!(job.state, JobState::Queued);

    // Only one worker should actually get a job
    let (res1, res2) = tokio::join!(worker1.run_once(), worker2.run_once());

    let processed1 = res1.unwrap();
    let processed2 = res2.unwrap();

    assert!(processed1 != processed2); // One true, one false
}

#[tokio::test]
async fn test_terminal_immutability() {
    let store = Arc::new(Mutex::new(JobStore::in_memory().unwrap()));
    let service = OperationService::new(store.clone());

    let op = Operation::SystemInfo(SystemInfoPayload {});
    let id = service.enqueue_operation(op).unwrap();

    let client = Arc::new(MockAgentClient {
        delay: Duration::from_millis(0),
        should_fail: true,
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
    assert_eq!(job.state, JobState::Failed);

    // Try to cancel a failed job, should error
    let res = service.cancel_operation(&id);
    assert!(res.is_err());
    let err_str = res.unwrap_err().to_string();
    assert!(err_str.contains("Invalid state transition from Failed to Cancelled") || err_str.contains("Invalid state transition"));
}

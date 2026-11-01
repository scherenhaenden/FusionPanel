#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;

    use fusionpanel_jobs::{JobState, JobStore, JobStoreError};

    #[test]
    fn test_schema_init_and_create() {
        let store = JobStore::in_memory().unwrap();

        let payload = json!({"action": "test"});
        let id = store.create("test_job", &payload).unwrap();

        let job = store.get(&id).unwrap();
        assert_eq!(job.id, id);
        assert_eq!(job.kind, "test_job");
        assert_eq!(job.payload, payload);
        assert_eq!(job.state, JobState::Queued);
        assert!(job.worker_id.is_none());
        assert!(job.error_text.is_none());
    }

    #[test]
    fn test_reopen_persistence() {
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_jobs_{}.db", uuid::Uuid::new_v4()));

        let payload = json!({"test": "data"});
        let id = {
            let store = JobStore::new(&db_path).unwrap();
            store.create("persistent_job", &payload).unwrap()
        };

        {
            let store = JobStore::new(&db_path).unwrap();
            let job = store.get(&id).unwrap();
            assert_eq!(job.kind, "persistent_job");
            assert_eq!(job.payload, payload);
        }

        std::fs::remove_file(db_path).unwrap();
    }

    #[test]
    fn test_claim_and_transition() {
        let mut store = JobStore::in_memory().unwrap();

        let id1 = store.create("job1", &json!({})).unwrap();

        // Let time pass to ensure ordering
        std::thread::sleep(Duration::from_millis(10));
        let id2 = store.create("job2", &json!({})).unwrap();

        // Should claim oldest
        let claimed = store.claim("worker1").unwrap().unwrap();
        assert_eq!(claimed.id, id1);
        assert_eq!(claimed.state, JobState::Running);
        assert_eq!(claimed.worker_id.as_deref(), Some("worker1"));

        // Second claim
        let claimed2 = store.claim("worker2").unwrap().unwrap();
        assert_eq!(claimed2.id, id2);
        assert_eq!(claimed2.state, JobState::Running);
        assert_eq!(claimed2.worker_id.as_deref(), Some("worker2"));

        // No more jobs
        assert!(store.claim("worker3").unwrap().is_none());

        // Valid transition
        store.transition(&id1, JobState::Succeeded, None).unwrap();
        let job1 = store.get(&id1).unwrap();
        assert_eq!(job1.state, JobState::Succeeded);

        // Valid transition to failed
        store
            .transition(&id2, JobState::Failed, Some("error msg".to_string()))
            .unwrap();
        let job2 = store.get(&id2).unwrap();
        assert_eq!(job2.state, JobState::Failed);
        assert_eq!(job2.error_text.as_deref(), Some("error msg"));
    }

    #[test]
    fn test_rejected_transitions() {
        let mut store = JobStore::in_memory().unwrap();
        let id = store.create("job", &json!({})).unwrap();

        // Queued -> Succeeded (rejected)
        let res = store.transition(&id, JobState::Succeeded, None);
        assert!(matches!(
            res,
            Err(JobStoreError::InvalidTransition {
                from: JobState::Queued,
                to: JobState::Succeeded
            })
        ));

        // Queued -> Running (allowed)
        store.transition(&id, JobState::Running, None).unwrap();

        // Running -> Queued (rejected)
        let res = store.transition(&id, JobState::Queued, None);
        assert!(matches!(
            res,
            Err(JobStoreError::InvalidTransition {
                from: JobState::Running,
                to: JobState::Queued
            })
        ));

        // Running -> Succeeded (allowed)
        store.transition(&id, JobState::Succeeded, None).unwrap();

        // Terminal -> Terminal (rejected)
        let res = store.transition(&id, JobState::Failed, None);
        assert!(matches!(
            res,
            Err(JobStoreError::InvalidTransition {
                from: JobState::Succeeded,
                to: JobState::Failed
            })
        ));
    }

    #[test]
    fn test_concurrent_claim() {
        // We simulate this sequentially by opening the same file twice,
        // which represents different connections (workers).
        let temp_dir = std::env::temp_dir();
        let db_path = temp_dir.join(format!("test_jobs_concurrent_{}.db", uuid::Uuid::new_v4()));

        let mut store1 = JobStore::new(&db_path).unwrap();
        let mut store2 = JobStore::new(&db_path).unwrap();

        store1.create("job", &json!({})).unwrap();

        let claim1 = store1.claim("worker1").unwrap();
        assert!(claim1.is_some());

        // Worker 2 should get nothing as job is already running
        let claim2 = store2.claim("worker2").unwrap();
        assert!(claim2.is_none());

        std::fs::remove_file(db_path).unwrap();
    }
}

#![forbid(unsafe_code)]

use fusionpanel_jobs::{JobState, JobStore, JobStoreError, MAX_ERROR_TEXT_BYTES};
use rusqlite::Connection;
use serde_json::json;
use std::sync::{Arc, Barrier};

#[test]
fn oversized_error_is_rejected_without_mutating_job() {
    let mut store = JobStore::in_memory().unwrap();
    let id = store.create("test", &json!({})).unwrap();
    store.claim("worker").unwrap().unwrap();
    let before = store.get(&id).unwrap();
    // Multibyte text must be bounded by storage bytes, not character count.
    let text = "é".repeat(MAX_ERROR_TEXT_BYTES / 2 + 1);
    let actual_bytes = text.len();
    let result = store.transition(&id, JobState::Failed, Some(text));
    assert!(
        matches!(result, Err(JobStoreError::ErrorTextTooLong { max_bytes: MAX_ERROR_TEXT_BYTES, actual_bytes: count }) if count == actual_bytes)
    );
    let after = store.get(&id).unwrap();
    assert_eq!(after.state, before.state);
    assert_eq!(after.updated_at, before.updated_at);
    assert_eq!(after.worker_id, before.worker_id);
    assert_eq!(after.error_text, before.error_text);
    let boundary = "x".repeat(MAX_ERROR_TEXT_BYTES);
    store
        .transition(&id, JobState::Failed, Some(boundary.clone()))
        .unwrap();
    assert_eq!(store.get(&id).unwrap().error_text, Some(boundary));
}

#[test]
fn malformed_claim_rolls_back_state_and_worker() {
    let db = std::env::temp_dir().join(format!("jobs_claim_malformed_{}.db", uuid::Uuid::new_v4()));
    let mut store = JobStore::new(&db).unwrap();
    let id = store.create("test", &json!({})).unwrap();
    let conn = Connection::open(&db).unwrap();
    conn.execute(
        "UPDATE jobs SET payload = 'not valid JSON' WHERE id = ?1",
        [&id],
    )
    .unwrap();
    let before: (String, Option<String>, String) = conn
        .query_row(
            "SELECT state, worker_id, updated_at FROM jobs WHERE id = ?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert!(store.claim("worker").is_err());
    let after: (String, Option<String>, String) = conn
        .query_row(
            "SELECT state, worker_id, updated_at FROM jobs WHERE id = ?1",
            [&id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(after, before);
    assert_eq!(after.0, "queued");
    assert_eq!(after.1, None);
    drop(conn);
    drop(store);
    std::fs::remove_file(db).unwrap();
}

#[test]
fn simultaneous_claimers_have_exactly_one_winner() {
    let db = std::env::temp_dir().join(format!("jobs_claim_race_{}.db", uuid::Uuid::new_v4()));
    let store = JobStore::new(&db).unwrap();
    let id = store.create("test", &json!({})).unwrap();
    drop(store);
    let barrier = Arc::new(Barrier::new(8));
    let workers: Vec<_> = (0..8)
        .map(|index| {
            let db = db.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                let mut store = JobStore::new(db).unwrap();
                barrier.wait();
                store.claim(&format!("worker-{index}")).unwrap()
            })
        })
        .collect();
    let claimed: Vec<_> = workers
        .into_iter()
        .filter_map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(claimed.len(), 1);
    assert_eq!(claimed[0].id, id);
    assert_eq!(claimed[0].state, JobState::Running);
    assert!(claimed[0]
        .worker_id
        .as_ref()
        .unwrap()
        .starts_with("worker-"));
    std::fs::remove_file(db).unwrap();
}

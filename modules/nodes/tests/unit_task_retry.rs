#![forbid(unsafe_code)]

use chrono::{Duration, Utc};
use nodes::task_retry::{ErrorClassification, Task, TaskError, TaskState};

#[test]
fn normal_completion() {
    let mut task = Task::new("task-1".to_string(), 3, 100, 1000);
    assert!(matches!(task.state, TaskState::Pending));

    task.start();
    assert!(matches!(task.state, TaskState::InProgress));

    task.complete();
    assert!(matches!(task.state, TaskState::Completed));
}

#[test]
fn transient_error_retries_and_exponential_backoff_with_full_jitter() {
    let mut task = Task::new("task-2".to_string(), 3, 100, 1000);
    task.start();

    let err = TaskError {
        message: "Network timeout".to_string(),
        classification: ErrorClassification::Transient,
    };

    let now = Utc::now();
    task.fail(err.clone(), 0, now, 0.5);

    match task.state {
        TaskState::Retrying { attempt, next_retry_at } => {
            assert_eq!(attempt, 1);
            let expected_delay = Duration::milliseconds((100.0 * 0.5) as i64);
            assert_eq!(next_retry_at, now + expected_delay);
        }
        _ => panic!("Expected Retrying state"),
    }

    // Try starting from Retrying
    task.start();
    assert!(matches!(task.state, TaskState::InProgress));

    // Next retry
    task.fail(err, 1, now, 1.0);
    match task.state {
        TaskState::Retrying { attempt, next_retry_at } => {
            assert_eq!(attempt, 2);
            let expected_delay = Duration::milliseconds(200); // 100 * 2^1 * 1.0
            assert_eq!(next_retry_at, now + expected_delay);
        }
        _ => panic!("Expected Retrying state"),
    }
}

#[test]
fn transient_errors_exceeding_max_retries_transitions_to_dlq() {
    let mut task = Task::new("task-3".to_string(), 2, 100, 1000);
    task.start();

    let err = TaskError {
        message: "Network timeout".to_string(),
        classification: ErrorClassification::Transient,
    };

    let now = Utc::now();

    // Retry 1
    task.fail(err.clone(), 0, now, 1.0);
    assert!(matches!(task.state, TaskState::Retrying { attempt: 1, .. }));

    task.start();

    // Retry 2
    task.fail(err.clone(), 1, now, 1.0);
    assert!(matches!(task.state, TaskState::Retrying { attempt: 2, .. }));

    task.start();

    // Exceed max retries
    task.fail(err.clone(), 2, now, 1.0);

    match task.state {
        TaskState::DeadLetterQueue { error } => {
            assert_eq!(error.message, "Network timeout");
            assert_eq!(error.classification, ErrorClassification::Transient);
        }
        _ => panic!("Expected DLQ state"),
    }
}

#[test]
fn fatal_errors_immediately_transition_to_dlq() {
    let mut task = Task::new("task-4".to_string(), 3, 100, 1000);
    task.start();

    let err = TaskError {
        message: "Corrupt data".to_string(),
        classification: ErrorClassification::Fatal,
    };

    let now = Utc::now();
    task.fail(err.clone(), 0, now, 1.0);

    match task.state {
        TaskState::DeadLetterQueue { error } => {
            assert_eq!(error.message, "Corrupt data");
            assert_eq!(error.classification, ErrorClassification::Fatal);
        }
        _ => panic!("Expected DLQ state"),
    }
}

#[test]
fn calculate_backoff_caps_at_max_backoff() {
    let mut task = Task::new("task-5".to_string(), 5, 100, 500);

    // 100 * 2^4 = 1600 > 500
    // Attempting attempt = 4 (5th attempt)
    // Jitter 1.0 => should be 500ms
    task.start(); // Doesn't matter for this test, but just to make sure we can trigger logic if needed.

    // Calculate through fail
    let mut t2 = task.clone();
    t2.start();

    let err = TaskError {
        message: "test".to_string(),
        classification: ErrorClassification::Transient,
    };

    let now = Utc::now();
    t2.fail(err, 4, now, 1.0);

    match t2.state {
        TaskState::Retrying { attempt, next_retry_at } => {
            assert_eq!(attempt, 5);
            let expected_delay = Duration::milliseconds(500);
            assert_eq!(next_retry_at, now + expected_delay);
        }
        _ => panic!("Expected Retrying state"),
    }
}

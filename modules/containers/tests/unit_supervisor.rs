#![forbid(unsafe_code)]

use containers::supervisor::{ContainerHealthSupervisor, RestartPolicy, SupervisorAction};
use std::time::Duration;

#[test]
fn test_no_restart_policy() {
    let mut supervisor = ContainerHealthSupervisor::new(RestartPolicy::NoRestart);

    assert!(matches!(supervisor.transition(false), SupervisorAction::None));
    assert_eq!(supervisor.consecutive_failures, 1);
    assert_eq!(supervisor.restart_attempts, 0);

    assert!(matches!(supervisor.transition(false), SupervisorAction::None));
    assert_eq!(supervisor.consecutive_failures, 2);
    assert_eq!(supervisor.restart_attempts, 0);

    // Transition back to healthy resets counters
    assert!(matches!(supervisor.transition(true), SupervisorAction::None));
    assert_eq!(supervisor.consecutive_failures, 0);
    assert_eq!(supervisor.restart_attempts, 0);
}

#[test]
fn test_always_restart_policy() {
    let mut supervisor = ContainerHealthSupervisor::new(RestartPolicy::Always);

    match supervisor.transition(false) {
        SupervisorAction::Restart { delay } => assert_eq!(delay, Duration::from_secs(0)),
        _ => panic!("Expected restart action"),
    }
    assert_eq!(supervisor.consecutive_failures, 1);
    assert_eq!(supervisor.restart_attempts, 1);

    match supervisor.transition(false) {
        SupervisorAction::Restart { delay } => assert_eq!(delay, Duration::from_secs(0)),
        _ => panic!("Expected restart action"),
    }
    assert_eq!(supervisor.consecutive_failures, 2);
    assert_eq!(supervisor.restart_attempts, 2);

    // Transition back to healthy resets counters
    assert!(matches!(supervisor.transition(true), SupervisorAction::None));
    assert_eq!(supervisor.consecutive_failures, 0);
    assert_eq!(supervisor.restart_attempts, 0);
}

#[test]
fn test_on_failure_policy_backoff() {
    let policy = RestartPolicy::OnFailure {
        max_retries: 4,
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(5),
    };
    let mut supervisor = ContainerHealthSupervisor::new(policy);

    // Attempt 0: delay = 1 * 2^0 = 1s
    match supervisor.transition(false) {
        SupervisorAction::Restart { delay } => assert_eq!(delay, Duration::from_secs(1)),
        _ => panic!("Expected restart action"),
    }
    assert_eq!(supervisor.restart_attempts, 1);
    assert_eq!(supervisor.consecutive_failures, 1);

    // Attempt 1: delay = 1 * 2^1 = 2s
    match supervisor.transition(false) {
        SupervisorAction::Restart { delay } => assert_eq!(delay, Duration::from_secs(2)),
        _ => panic!("Expected restart action"),
    }
    assert_eq!(supervisor.restart_attempts, 2);
    assert_eq!(supervisor.consecutive_failures, 2);

    // Attempt 2: delay = 1 * 2^2 = 4s
    match supervisor.transition(false) {
        SupervisorAction::Restart { delay } => assert_eq!(delay, Duration::from_secs(4)),
        _ => panic!("Expected restart action"),
    }
    assert_eq!(supervisor.restart_attempts, 3);
    assert_eq!(supervisor.consecutive_failures, 3);

    // Attempt 3: delay = 1 * 2^3 = 8s, capped at max_delay = 5s
    match supervisor.transition(false) {
        SupervisorAction::Restart { delay } => assert_eq!(delay, Duration::from_secs(5)),
        _ => panic!("Expected restart action"),
    }
    assert_eq!(supervisor.restart_attempts, 4);
    assert_eq!(supervisor.consecutive_failures, 4);

    // Attempt 4: max retries reached, no restart
    assert!(matches!(supervisor.transition(false), SupervisorAction::None));
    // restart_attempts stays at 4 since no restart was initiated
    assert_eq!(supervisor.restart_attempts, 4);
    assert_eq!(supervisor.consecutive_failures, 5);

    // Back to healthy
    assert!(matches!(supervisor.transition(true), SupervisorAction::None));
    assert_eq!(supervisor.consecutive_failures, 0);
    assert_eq!(supervisor.restart_attempts, 0);
}

#[test]
fn test_on_failure_no_overflow() {
    let policy = RestartPolicy::OnFailure {
        max_retries: u32::MAX, // High retries to allow overflow condition in bitshift if unchecked
        base_delay: Duration::from_secs(1),
        max_delay: Duration::from_secs(100),
    };
    let mut supervisor = ContainerHealthSupervisor::new(policy);

    supervisor.restart_attempts = 35; // 1 << 35 will overflow u32

    match supervisor.transition(false) {
        SupervisorAction::Restart { delay } => assert_eq!(delay, Duration::from_secs(100)), // Should cap at max_delay instead of panicking
        _ => panic!("Expected restart action"),
    }
    assert_eq!(supervisor.restart_attempts, 36);
}

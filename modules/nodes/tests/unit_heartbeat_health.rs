#![forbid(unsafe_code)]

use nodes::heartbeat_health::{NodeHealthConfig, NodeHealthTracker, NodeStatus};

#[test]
fn test_initial_state() {
    let tracker = NodeHealthTracker::new(NodeHealthConfig::default());
    assert_eq!(tracker.status(), NodeStatus::Healthy);
    assert_eq!(tracker.health_score(), 100.0);
}

#[test]
fn test_health_score_deductions() {
    let mut tracker = NodeHealthTracker::new(NodeHealthConfig::default());

    // CPU > 70 starts deducting (load - 70) * 0.5
    // RAM > 80 starts deducting (util - 80) * 1.0

    // CPU 90 (deduct 10), RAM 90 (deduct 10)
    tracker.record_successful_heartbeat(90.0, 90.0);
    assert_eq!(tracker.health_score(), 80.0);

    // CPU 100 (deduct 15), RAM 100 (deduct 20)
    tracker.record_successful_heartbeat(100.0, 100.0);
    assert_eq!(tracker.health_score(), 65.0);
}

#[test]
fn test_status_transitions_missed_heartbeats() {
    let mut config = NodeHealthConfig::default();
    config.degraded_missed_heartbeats = 1;
    config.unreachable_missed_heartbeats = 3;
    config.quarantined_missed_heartbeats = 5;

    let mut tracker = NodeHealthTracker::new(config);

    // 1 missed heartbeat -> Degraded
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Degraded);
    assert_eq!(tracker.health_score(), 90.0); // 10 points deducted for 1 missed heartbeat

    // 2 missed heartbeats -> Still Degraded
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Degraded);
    assert_eq!(tracker.health_score(), 80.0);

    // 3 missed heartbeats -> Unreachable
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Unreachable);
    assert_eq!(tracker.health_score(), 0.0); // Unreachable zeroes score

    // 4 missed heartbeats -> Still Unreachable
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Unreachable);

    // 5 missed heartbeats -> Quarantined
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Quarantined);
    assert_eq!(tracker.health_score(), 0.0);
}

#[test]
fn test_recovery_from_degraded() {
    let mut config = NodeHealthConfig::default();
    config.recovery_success_threshold = 2; // Needs 2 successes to recover

    let mut tracker = NodeHealthTracker::new(config);

    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Degraded);

    // 1 success -> Not enough to recover to Healthy yet
    tracker.record_successful_heartbeat(50.0, 50.0);
    assert_eq!(tracker.status(), NodeStatus::Degraded);

    // 2 successes -> Recover to Healthy
    tracker.record_successful_heartbeat(50.0, 50.0);
    assert_eq!(tracker.status(), NodeStatus::Healthy);
    assert_eq!(tracker.health_score(), 100.0);
}

#[test]
fn test_recovery_from_unreachable() {
    let mut config = NodeHealthConfig::default();
    config.unreachable_missed_heartbeats = 2;
    config.recovery_success_threshold = 3;

    let mut tracker = NodeHealthTracker::new(config);

    tracker.record_missed_heartbeat();
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Unreachable);

    tracker.record_successful_heartbeat(50.0, 50.0);
    assert_eq!(tracker.status(), NodeStatus::Unreachable); // Still Unreachable

    tracker.record_successful_heartbeat(50.0, 50.0);
    assert_eq!(tracker.status(), NodeStatus::Unreachable);

    tracker.record_successful_heartbeat(50.0, 50.0);
    assert_eq!(tracker.status(), NodeStatus::Healthy); // Recovered!
}

#[test]
fn test_quarantine_no_auto_recovery() {
    let mut config = NodeHealthConfig::default();
    config.quarantined_missed_heartbeats = 2;
    let mut tracker = NodeHealthTracker::new(config);

    tracker.record_missed_heartbeat();
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Quarantined);

    for _ in 0..10 {
        tracker.record_successful_heartbeat(50.0, 50.0);
    }

    // Status should remain Quarantined
    assert_eq!(tracker.status(), NodeStatus::Quarantined);
    assert_eq!(tracker.health_score(), 0.0);
}

#[test]
fn test_manual_quarantine() {
    let mut tracker = NodeHealthTracker::new(NodeHealthConfig::default());
    tracker.manual_quarantine();

    assert_eq!(tracker.status(), NodeStatus::Quarantined);
    assert_eq!(tracker.health_score(), 0.0);
}

#[test]
fn test_escalation_and_partial_recovery() {
    let mut config = NodeHealthConfig::default();
    config.degraded_missed_heartbeats = 1;
    config.unreachable_missed_heartbeats = 3;
    config.recovery_success_threshold = 3;

    let mut tracker = NodeHealthTracker::new(config);

    tracker.record_missed_heartbeat();
    tracker.record_missed_heartbeat();
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Unreachable);

    // One successful heartbeat -> not enough to recover
    tracker.record_successful_heartbeat(50.0, 50.0);
    assert_eq!(tracker.status(), NodeStatus::Unreachable);

    // One more missed heartbeat -> Should remain Unreachable, NOT downgrade to Degraded
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Unreachable);
}

#[test]
fn test_health_score_partial_recovery() {
    let mut config = NodeHealthConfig::default();
    config.degraded_missed_heartbeats = 2;
    config.recovery_success_threshold = 3;

    let mut tracker = NodeHealthTracker::new(config);
    tracker.record_missed_heartbeat();
    tracker.record_missed_heartbeat();
    assert_eq!(tracker.status(), NodeStatus::Degraded);
    assert_eq!(tracker.health_score(), 80.0);

    // One successful heartbeat -> consecutive misses reset, but status remains degraded.
    tracker.record_successful_heartbeat(50.0, 50.0);
    assert_eq!(tracker.status(), NodeStatus::Degraded);
    assert_eq!(tracker.health_score(), 80.0); // should penalize for degraded state until recovery
}

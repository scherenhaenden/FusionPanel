#![forbid(unsafe_code)]

use nodes::node_eviction::{
    EvictionError, EvictionEvent, NodeEvictionCoordinator, NodeLifecycleState,
};

#[test]
fn test_valid_state_transitions() {
    let mut coordinator = NodeEvictionCoordinator::new(2);

    assert_eq!(coordinator.state(), NodeLifecycleState::Active);

    // Active -> Quarantined
    assert!(coordinator.quarantine().is_ok());
    assert_eq!(coordinator.state(), NodeLifecycleState::Quarantined);

    // Quarantined -> Draining
    let event = coordinator.start_drain().unwrap();
    assert_eq!(event, EvictionEvent::WorkloadDrainRequested);
    assert_eq!(coordinator.state(), NodeLifecycleState::Draining);

    coordinator.complete_workload();
    coordinator.complete_workload();

    // Draining -> Evicted
    let events = coordinator.evict().unwrap();
    assert_eq!(
        events,
        vec![
            EvictionEvent::CertificateRevocationRequired,
            EvictionEvent::NodePurged
        ]
    );
    assert_eq!(coordinator.state(), NodeLifecycleState::Evicted);
}

#[test]
fn test_active_to_draining() {
    let mut coordinator = NodeEvictionCoordinator::new(0);

    // Active -> Draining is also valid?
    // Based on the code implemented, Active -> Draining is allowed. Let's test it.
    assert!(coordinator.start_drain().is_ok());
    assert_eq!(coordinator.state(), NodeLifecycleState::Draining);
}

#[test]
fn test_invalid_state_transition_quarantine_rejection() {
    let mut coordinator = NodeEvictionCoordinator::new(0);
    coordinator.quarantine().unwrap();
    coordinator.start_drain().unwrap();

    // Try to quarantine from draining state
    let result = coordinator.quarantine();
    assert!(matches!(
        result,
        Err(EvictionError::InvalidTransition {
            from: NodeLifecycleState::Draining,
            to: NodeLifecycleState::Quarantined,
        })
    ));
}

#[test]
fn test_invalid_state_transition_evict_from_active() {
    let mut coordinator = NodeEvictionCoordinator::new(0);

    let result = coordinator.evict();
    assert!(matches!(
        result,
        Err(EvictionError::InvalidTransition {
            from: NodeLifecycleState::Active,
            to: NodeLifecycleState::Evicted,
        })
    ));
}

#[test]
fn test_drain_progress_tracking_prevents_eviction() {
    let mut coordinator = NodeEvictionCoordinator::new(3);
    coordinator.quarantine().unwrap();
    coordinator.start_drain().unwrap();

    // Attempt evict with 3 workloads
    let result = coordinator.evict();
    assert!(matches!(
        result,
        Err(EvictionError::WorkloadsRemaining { workloads: 3 })
    ));

    coordinator.complete_workload();
    coordinator.complete_workload();

    // Attempt evict with 1 workload
    let result = coordinator.evict();
    assert!(matches!(
        result,
        Err(EvictionError::WorkloadsRemaining { workloads: 1 })
    ));

    coordinator.complete_workload();

    // Success with 0 workloads
    assert!(coordinator.evict().is_ok());
}

#![forbid(unsafe_code)]

use nodes::heartbeat_ring::{HeartbeatRing, HeartbeatRingError, NodeState, QuorumStatus};

#[test]
fn test_ring_generation_and_routing() {
    let mut ring = HeartbeatRing::new(3, 10);

    // Add 3 nodes
    ring.add_node("node1".to_string()).unwrap();
    ring.add_node("node2".to_string()).unwrap();
    ring.add_node("node3".to_string()).unwrap();

    // In BTreeMap order: node1, node2, node3
    assert_eq!(ring.successor("node1").unwrap(), "node2");
    assert_eq!(ring.successor("node2").unwrap(), "node3");
    assert_eq!(ring.successor("node3").unwrap(), "node1");

    // Add up to 10 nodes
    for i in 4..=10 {
        ring.add_node(format!("node{}", i)).unwrap();
    }

    // node1 -> node10 (because "node10" is before "node2" lexicographically)
    // Keys: node1, node10, node2, node3, node4, node5, node6, node7, node8, node9
    assert_eq!(ring.successor("node1").unwrap(), "node10");
    assert_eq!(ring.successor("node10").unwrap(), "node2");
    assert_eq!(ring.successor("node9").unwrap(), "node1");
}

#[test]
fn test_missed_heartbeat_escalation() {
    let mut ring = HeartbeatRing::new(2, 4);

    ring.add_node("n1".to_string()).unwrap();
    ring.add_node("n2".to_string()).unwrap();
    ring.add_node("n3".to_string()).unwrap();

    assert_eq!(ring.get_node("n2").unwrap().state, NodeState::Active);

    // Tick n2 1 time
    ring.tick_node("n2").unwrap();
    assert_eq!(ring.get_node("n2").unwrap().state, NodeState::Active);

    // Tick n2 to suspect threshold (2)
    ring.tick_node("n2").unwrap();
    assert_eq!(ring.get_node("n2").unwrap().state, NodeState::Suspected);

    // Tick n2 to dead threshold (4)
    ring.tick_node("n2").unwrap();
    ring.tick_node("n2").unwrap();
    assert_eq!(ring.get_node("n2").unwrap().state, NodeState::Dead);

    // After death, successor routing should skip it
    // Keys: n1, n2, n3. n1 successor should be n3
    assert_eq!(ring.successor("n1").unwrap(), "n3");

    // Send heartbeat to n2, should bring it back
    ring.receive_heartbeat("n2", 1).unwrap();
    assert_eq!(ring.get_node("n2").unwrap().state, NodeState::Active);
    assert_eq!(ring.successor("n1").unwrap(), "n2");
}

#[test]
fn test_majority_quorum_partition_detection() {
    let mut ring = HeartbeatRing::new(3, 5);

    ring.add_node("n1".to_string()).unwrap();
    ring.add_node("n2".to_string()).unwrap();
    ring.add_node("n3".to_string()).unwrap();
    ring.add_node("n4".to_string()).unwrap();
    ring.add_node("n5".to_string()).unwrap();

    assert_eq!(ring.check_quorum(), QuorumStatus::HasQuorum);

    // Kill 1 node (reachable = 4, > 5/2 (2))
    for _ in 0..5 {
        ring.tick_node("n1").unwrap();
    }
    assert_eq!(ring.check_quorum(), QuorumStatus::HasQuorum);

    // Kill 2 more nodes (total 3 dead) -> reachable = 2, not > 2
    for _ in 0..5 {
        ring.tick_node("n2").unwrap();
        ring.tick_node("n3").unwrap();
    }
    assert_eq!(ring.check_quorum(), QuorumStatus::IsolatedPartition);
}

#[test]
fn test_errors() {
    let mut ring = HeartbeatRing::default();

    assert_eq!(ring.add_node("n1".to_string()).unwrap(), ());
    assert_eq!(
        ring.add_node("n1".to_string()).unwrap_err(),
        HeartbeatRingError::NodeAlreadyExists("n1".to_string())
    );

    assert_eq!(
        ring.remove_node("n2").unwrap_err(),
        HeartbeatRingError::NodeNotFound("n2".to_string())
    );

    assert_eq!(
        ring.successor("n1").unwrap_err(),
        HeartbeatRingError::InsufficientNodes
    );
}

#[test]
fn test_tick_all() {
    let mut ring = HeartbeatRing::new(2, 4);
    ring.add_node("n1".to_string()).unwrap();
    ring.add_node("n2".to_string()).unwrap();

    ring.tick_all();
    ring.tick_all();

    assert_eq!(ring.get_node("n1").unwrap().state, NodeState::Suspected);
    assert_eq!(ring.get_node("n2").unwrap().state, NodeState::Suspected);
}

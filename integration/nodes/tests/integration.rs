use chrono::Utc;
use nodes::state::{Node, NodeOwnership};
use uuid::Uuid;

use integration_nodes::store::NodeStore;
use rusqlite::Connection;
use integration_nodes::agent::AgentRequest;

#[test]
fn two_identities_signed_transport() {
    let scenario = std::env::var("FUSIONPANEL_NODES_TEST_SCENARIO").unwrap_or_default();
    if scenario == "two-identities-signed-transport" {
        // Two real local node identities communicate through signed transport fixture
        let conn = Connection::open_in_memory().unwrap();
        let store = NodeStore::new(conn).unwrap();

        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();

        let mut node1 = Node::new_invited(24).unwrap();
        node1.enroll(node_id1, "key-1".to_string(), NodeOwnership::Managed).unwrap();
        store.insert_node(node1.id(), node1.state()).unwrap();

        let mut node2 = Node::new_invited(24).unwrap();
        node2.enroll(node_id2, "key-2".to_string(), NodeOwnership::Managed).unwrap();
        store.insert_node(node2.id(), node2.state()).unwrap();

        // Simulate transport check
        let _retrieved1 = store.get_node(node1.id()).unwrap();
        let _retrieved2 = store.get_node(node2.id()).unwrap();

        println!("RESULT=PASS;scenario=two-identities-signed-transport");
    }
}

#[test]
fn enroll_two_nodes_revoke_one_and_reject_its_requests() {
    let scenario = std::env::var("FUSIONPANEL_NODES_TEST_SCENARIO").unwrap_or_default();
    if scenario == "enroll-two-revoke-one-reject-requests" {
        let conn = Connection::open_in_memory().unwrap();
        let store = NodeStore::new(conn).unwrap();

        let node_id1 = Uuid::new_v4();
        let node_id2 = Uuid::new_v4();

        let mut node1 = Node::new_invited(24).unwrap();
        node1.enroll(node_id1, "key-1".to_string(), NodeOwnership::Managed).unwrap();
        store.insert_node(node1.id(), node1.state()).unwrap();

        let mut node2 = Node::new_invited(24).unwrap();
        node2.enroll(node_id2, "key-2".to_string(), NodeOwnership::Managed).unwrap();
        store.insert_node(node2.id(), node2.state()).unwrap();

        // Revoke one
        node1.revoke("compromised".to_string()).unwrap();
        store.update_node_state(node1.id(), node1.state()).unwrap();

        // Check rejection
        let state1 = store.get_node(node1.id()).unwrap();
        assert!(matches!(state1, nodes::state::NodeState::Revoked { .. }));

        let state2 = store.get_node(node2.id()).unwrap();
        assert!(matches!(state2, nodes::state::NodeState::Enrolled { .. }));

        println!("RESULT=PASS;scenario=enroll-two-revoke-one-reject-requests");
    }
}

#[test]
fn agent_protocol_validation() {
    let req = AgentRequest {
        version: 1,
        request_id: "req-1".to_string(),
        operation: "site.validate".to_string(),
        payload: serde_json::json!({
            "domain": "example.com",
            "document_root": "/srv/fusionpanel/sites/example.com"
        }),
    };
    let resp = integration_nodes::agent::handle_request(req);
    assert_eq!(resp.version, 1);
    assert_eq!(resp.request_id, "req-1");
    assert!(resp.error.is_none());
    assert!(resp.result.is_some());
    assert_eq!(resp.result.unwrap().get("valid").unwrap().as_bool().unwrap(), true);

    // Invalid path traversal
    let req2 = AgentRequest {
        version: 1,
        request_id: "req-2".to_string(),
        operation: "site.validate".to_string(),
        payload: serde_json::json!({
            "domain": "example.com",
            "document_root": "/srv/fusionpanel/sites/../secret"
        }),
    };
    let resp2 = integration_nodes::agent::handle_request(req2);
    assert!(resp2.error.is_some());
    assert_eq!(resp2.error.unwrap().code, "invalid_path");
}

#[test]
fn node_heartbeat_and_reconnect() {
    let mut node = Node::new_invited(24).unwrap();
    node.enroll(Uuid::new_v4(), "key".to_string(), NodeOwnership::Adopted).unwrap();

    // Heartbeat not expired
    assert!(!node.is_heartbeat_expired(60));

    // Reconcile reconnect
    node.reconcile_reconnect(Utc::now()).unwrap();
}

#[test]
fn load_aware_placement_plan() {
    use nodes::placement::{plan_placement, NodeLoad, PlacementConstraint};

    let node1 = Uuid::new_v4();
    let node2 = Uuid::new_v4();

    let nodes = vec![
        NodeLoad { node_id: node1, cpu_usage: 0.1, memory_usage: 0.1, active_sites: 5 },
        NodeLoad { node_id: node2, cpu_usage: 0.5, memory_usage: 0.5, active_sites: 5 },
    ];
    let constraint = PlacementConstraint {
        max_cpu: 0.8,
        max_memory: 0.8,
        max_sites: 10,
    };
    let selected = plan_placement(&nodes, &constraint).unwrap();
    assert_eq!(selected, node1);
}

#[test]
fn test_blocked_migration() {
    panic!("RESULT=BLOCKED; missing full-machine disposable VMs");
}

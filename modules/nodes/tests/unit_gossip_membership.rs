#![forbid(unsafe_code)]

use chrono::{Utc, Duration};
use nodes::gossip_membership::{GossipMembership, GossipConfig, NodeState};

#[test]
fn test_node_lifecycle_and_timeouts() {
    let config = GossipConfig {
        suspect_timeout_secs: 5,
        dead_timeout_secs: 30,
    };
    let mut gossip = GossipMembership::new(config);
    let mut now = Utc::now();
    let node_id = "node-1".to_string();

    // 1. Initial add
    gossip.update_node(node_id.clone(), NodeState::Alive, 1, now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Alive);
    assert_eq!(node.incarnation, 1);

    // 2. Network blip -> Suspect (same incarnation)
    now += Duration::seconds(10);
    gossip.update_node(node_id.clone(), NodeState::Suspect, 1, now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Suspect);

    // 3. Tick does not transition if time hasn't passed
    now += Duration::seconds(2);
    gossip.tick(now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Suspect); // still suspect

    // 4. Tick transitions to Dead after suspect timeout
    now += Duration::seconds(4);
    gossip.tick(now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Dead);

    // 5. Tick transitions to Left after dead timeout
    now += Duration::seconds(31);
    gossip.tick(now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Left);
}

#[test]
fn test_state_precedence_same_incarnation() {
    let mut gossip = GossipMembership::new(GossipConfig::default());
    let now = Utc::now();
    let node_id = "node-2".to_string();

    gossip.update_node(node_id.clone(), NodeState::Suspect, 1, now);

    // Alive is lower than Suspect, should not override
    gossip.update_node(node_id.clone(), NodeState::Alive, 1, now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Suspect);

    // Dead is higher than Suspect, should override
    gossip.update_node(node_id.clone(), NodeState::Dead, 1, now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Dead);
}

#[test]
fn test_incarnation_override() {
    let mut gossip = GossipMembership::new(GossipConfig::default());
    let now = Utc::now();
    let node_id = "node-3".to_string();

    // Current state is Dead
    gossip.update_node(node_id.clone(), NodeState::Dead, 1, now);

    // Higher incarnation with Alive state should override Dead state
    gossip.update_node(node_id.clone(), NodeState::Alive, 2, now);
    let node = gossip.get_node(&node_id).unwrap();
    assert_eq!(node.state, NodeState::Alive);
    assert_eq!(node.incarnation, 2);
}

#[test]
fn test_get_active_nodes() {
    let mut gossip = GossipMembership::new(GossipConfig::default());
    let now = Utc::now();

    gossip.update_node("node-a".to_string(), NodeState::Alive, 1, now);
    gossip.update_node("node-b".to_string(), NodeState::Suspect, 1, now);
    gossip.update_node("node-c".to_string(), NodeState::Alive, 1, now);
    gossip.update_node("node-d".to_string(), NodeState::Dead, 1, now);

    let active = gossip.get_active_nodes();
    assert_eq!(active.len(), 2);

    let mut ids: Vec<_> = active.iter().map(|n| n.id.as_str()).collect();
    ids.sort();
    assert_eq!(ids, vec!["node-a", "node-c"]);
}

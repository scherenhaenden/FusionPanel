#![forbid(unsafe_code)]

use nodes::ping_matrix::{PingMatrix, RttStats};
use std::time::Duration;

#[test]
fn test_rtt_stats_average() {
    let mut stats = RttStats::new(3);

    stats.add_sample(Duration::from_millis(10));
    assert_eq!(stats.average_rtt, Duration::from_millis(10));

    stats.add_sample(Duration::from_millis(20));
    assert_eq!(stats.average_rtt, Duration::from_millis(15));

    stats.add_sample(Duration::from_millis(30));
    assert_eq!(stats.average_rtt, Duration::from_millis(20));

    // Test sliding window, should drop the 10ms sample
    stats.add_sample(Duration::from_millis(40));
    assert_eq!(stats.average_rtt, Duration::from_millis(30)); // (20+30+40)/3 = 30
}

#[test]
fn test_ping_matrix_update_and_get() {
    let mut matrix = PingMatrix::new(5);
    matrix.update("node_a", "node_b", Duration::from_millis(50));
    matrix.update("node_a", "node_b", Duration::from_millis(60));

    assert_eq!(
        matrix.get_average_rtt("node_a", "node_b"),
        Some(Duration::from_millis(55))
    );
    assert_eq!(matrix.get_average_rtt("node_b", "node_a"), None);
}

#[test]
fn test_ping_matrix_isolated_peers() {
    let mut matrix = PingMatrix::new(5);

    matrix.add_node("node_a");
    matrix.add_node("node_b");
    matrix.add_node("node_c"); // This node will be isolated

    matrix.update("node_a", "node_b", Duration::from_millis(10));

    let isolated = matrix.isolated_peers();
    assert_eq!(isolated, vec!["node_c"]);

    // Add an edge from node_c
    matrix.update("node_c", "node_a", Duration::from_millis(15));
    let isolated2 = matrix.isolated_peers();
    assert!(isolated2.is_empty());
}

#[test]
fn test_ping_matrix_high_latency_peers() {
    let mut matrix = PingMatrix::new(5);

    matrix.update("node_a", "node_b", Duration::from_millis(20));
    matrix.update("node_a", "node_c", Duration::from_millis(150));
    matrix.update("node_b", "node_c", Duration::from_millis(80));

    let threshold = Duration::from_millis(100);
    let high_latency = matrix.high_latency_peers(threshold);

    assert_eq!(high_latency, vec![("node_a".to_string(), "node_c".to_string())]);
}

#[test]
fn test_ping_matrix_remove_node() {
    let mut matrix = PingMatrix::new(5);

    matrix.update("node_a", "node_b", Duration::from_millis(20));
    matrix.update("node_a", "node_c", Duration::from_millis(30));
    matrix.update("node_c", "node_a", Duration::from_millis(40));

    matrix.remove_node("node_c");

    assert_eq!(matrix.get_average_rtt("node_a", "node_c"), None);
    assert_eq!(matrix.get_average_rtt("node_c", "node_a"), None);
    assert_eq!(matrix.get_average_rtt("node_a", "node_b"), Some(Duration::from_millis(20)));

    let isolated = matrix.isolated_peers();
    assert!(isolated.is_empty()); // Node a and b are connected
}

#[test]
fn test_ping_matrix_remove_edge() {
    let mut matrix = PingMatrix::new(5);

    matrix.update("node_a", "node_b", Duration::from_millis(20));
    matrix.update("node_a", "node_c", Duration::from_millis(30));

    matrix.remove_edge("node_a", "node_b");

    assert_eq!(matrix.get_average_rtt("node_a", "node_b"), None);
    assert_eq!(matrix.get_average_rtt("node_a", "node_c"), Some(Duration::from_millis(30)));
}

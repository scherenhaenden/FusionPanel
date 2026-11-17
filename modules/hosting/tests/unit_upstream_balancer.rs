#![forbid(unsafe_code)]

use hosting::upstream_balancer::{BalancingStrategy, UpstreamBackend, UpstreamLoadBalancer};

#[test]
fn test_round_robin_rotation() {
    let backends = vec![
        UpstreamBackend::new("127.0.0.1:8080", 1),
        UpstreamBackend::new("127.0.0.1:8081", 1),
        UpstreamBackend::new("127.0.0.1:8082", 1),
    ];
    let mut balancer = UpstreamLoadBalancer::new(backends, BalancingStrategy::RoundRobin);

    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8081");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8082");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8081");
}

#[test]
fn test_least_connections_distribution() {
    let backends = vec![
        UpstreamBackend::new("127.0.0.1:8080", 1),
        UpstreamBackend::new("127.0.0.1:8081", 1),
    ];
    let mut balancer = UpstreamLoadBalancer::new(backends, BalancingStrategy::LeastConnections);

    // Initial state: both have 0 connections
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");
    // 8080 has 1 connection, 8081 has 0
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8081");
    // Both have 1 connection, tie break goes to the first in the list (8080)
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");

    // Check internal state directly
    assert_eq!(balancer.backends[0].active_connections, 2);
    assert_eq!(balancer.backends[1].active_connections, 1);

    // Release connection on 8080
    balancer.release("127.0.0.1:8080");
    assert_eq!(balancer.backends[0].active_connections, 1);

    // Now both have 1, it should pick 8080 again
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");
}

#[test]
fn test_ip_hash_stickiness() {
    let backends = vec![
        UpstreamBackend::new("127.0.0.1:8080", 1),
        UpstreamBackend::new("127.0.0.1:8081", 1),
        UpstreamBackend::new("127.0.0.1:8082", 1),
    ];
    let mut balancer = UpstreamLoadBalancer::new(backends, BalancingStrategy::IpHash);

    // Same IP should give the same backend
    let backend1 = balancer.dispatch(Some("192.168.1.100")).unwrap();
    let backend2 = balancer.dispatch(Some("192.168.1.100")).unwrap();
    assert_eq!(backend1, backend2);

    // Another IP might give a different one (or the same, but it should be consistent)
    let backend3 = balancer.dispatch(Some("10.0.0.5")).unwrap();
    let backend4 = balancer.dispatch(Some("10.0.0.5")).unwrap();
    assert_eq!(backend3, backend4);

    // Test fallback to round-robin if no IP provided
    let backend5 = balancer.dispatch(None).unwrap();
    let backend6 = balancer.dispatch(None).unwrap();
    assert_ne!(backend5, backend6);
}

#[test]
fn test_unhealthy_backend_exclusion() {
    let mut backends = vec![
        UpstreamBackend::new("127.0.0.1:8080", 1),
        UpstreamBackend::new("127.0.0.1:8081", 1),
        UpstreamBackend::new("127.0.0.1:8082", 1),
    ];

    // Mark the second backend as unhealthy
    backends[1].is_healthy = false;

    let mut balancer = UpstreamLoadBalancer::new(backends, BalancingStrategy::RoundRobin);

    // It should skip 8081
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8082");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");

    // Try with LeastConnections
    balancer.strategy = BalancingStrategy::LeastConnections;
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8082");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8080");
    assert_eq!(balancer.dispatch(None).unwrap(), "127.0.0.1:8082");
}

#[test]
fn test_all_unhealthy() {
    let mut backends = vec![
        UpstreamBackend::new("127.0.0.1:8080", 1),
        UpstreamBackend::new("127.0.0.1:8081", 1),
    ];
    backends[0].is_healthy = false;
    backends[1].is_healthy = false;

    let mut balancer = UpstreamLoadBalancer::new(backends, BalancingStrategy::RoundRobin);
    assert_eq!(balancer.dispatch(None), None);
}

#[test]
fn test_active_connections_tracking() {
    let backends = vec![UpstreamBackend::new("127.0.0.1:8080", 1)];
    let mut balancer = UpstreamLoadBalancer::new(backends, BalancingStrategy::RoundRobin);

    assert_eq!(balancer.backends[0].active_connections, 0);

    balancer.dispatch(None).unwrap();
    assert_eq!(balancer.backends[0].active_connections, 1);

    balancer.dispatch(None).unwrap();
    assert_eq!(balancer.backends[0].active_connections, 2);

    balancer.release("127.0.0.1:8080");
    assert_eq!(balancer.backends[0].active_connections, 1);

    balancer.release("127.0.0.1:8080");
    assert_eq!(balancer.backends[0].active_connections, 0);

    // Releasing beyond 0 should not underflow (handled by saturating_sub)
    balancer.release("127.0.0.1:8080");
    assert_eq!(balancer.backends[0].active_connections, 0);
}

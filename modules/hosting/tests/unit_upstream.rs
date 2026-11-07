#![forbid(unsafe_code)]

use hosting::domain::error::DomainError;
use hosting::upstream::{
    generate_health_check_directive, generate_upstream_pool, ActiveHealthCheck, MatchBlock,
    UpstreamPoolConfig, UpstreamServer,
};

#[test]
fn test_generate_basic_upstream_pool() {
    let config = UpstreamPoolConfig {
        name: "backend_pool".to_string(),
        servers: vec![
            UpstreamServer {
                address: "10.0.0.1:8080".to_string(),
                max_fails: Some(3),
                fail_timeout: Some("30s".to_string()),
                backup: false,
                weight: Some(2),
            },
            UpstreamServer {
                address: "10.0.0.2:8080".to_string(),
                max_fails: None,
                fail_timeout: None,
                backup: true,
                weight: None,
            },
        ],
        keepalive: Some(32),
        zone_name: Some("backend_zone".to_string()),
        zone_size: Some("1m".to_string()),
        active_check: None,
        match_blocks: vec![],
    };

    let result = generate_upstream_pool(&config).expect("Should generate successfully");

    assert!(result.contains("upstream backend_pool {"));
    assert!(result.contains("    zone backend_zone 1m;"));
    assert!(result.contains("    server 10.0.0.1:8080 weight=2 max_fails=3 fail_timeout=30s;"));
    assert!(result.contains("    server 10.0.0.2:8080 backup;"));
    assert!(result.contains("    keepalive 32;"));
    assert!(result.ends_with("}\n"));
}

#[test]
fn test_generate_upstream_pool_with_match_blocks() {
    let config = UpstreamPoolConfig {
        name: "my_pool".to_string(),
        servers: vec![UpstreamServer {
            address: "unix:/tmp/test.sock".to_string(),
            max_fails: None,
            fail_timeout: None,
            backup: false,
            weight: None,
        }],
        keepalive: None,
        zone_name: None,
        zone_size: None,
        active_check: None,
        match_blocks: vec![MatchBlock {
            name: "server_ok".to_string(),
            statuses: vec!["200".to_string(), "201-299".to_string()],
            body_regex: Some("healthz: ok".to_string()),
        }],
    };

    let result = generate_upstream_pool(&config).expect("Should generate successfully");
    assert!(result.contains("upstream my_pool {"));
    assert!(result.contains("    server unix:/tmp/test.sock;"));
    assert!(result.contains("match server_ok {"));
    assert!(result.contains("    status 200 201-299;"));
    assert!(result.contains("    body ~ \"healthz: ok\";"));
}

#[test]
fn test_invalid_upstream_pool_name() {
    let config = UpstreamPoolConfig {
        name: "backend pool!".to_string(), // Invalid chars
        servers: vec![UpstreamServer {
            address: "127.0.0.1".to_string(),
            max_fails: None,
            fail_timeout: None,
            backup: false,
            weight: None,
        }],
        ..Default::default()
    };

    let err = generate_upstream_pool(&config).unwrap_err();
    assert!(matches!(err, DomainError::InvalidInput(_)));
}

#[test]
fn test_invalid_server_address() {
    let config = UpstreamPoolConfig {
        name: "backend".to_string(),
        servers: vec![UpstreamServer {
            address: "127.0.0.1; rm -rf /".to_string(), // Injection attempt
            max_fails: None,
            fail_timeout: None,
            backup: false,
            weight: None,
        }],
        ..Default::default()
    };

    let err = generate_upstream_pool(&config).unwrap_err();
    assert!(matches!(err, DomainError::InvalidInput(_)));
}

#[test]
fn test_generate_health_check_directive() {
    let check = ActiveHealthCheck {
        interval: Some("10s".to_string()),
        passes: Some(2),
        fails: Some(3),
        uri: Some("/health".to_string()),
        match_name: Some("server_ok".to_string()),
        port: Some(8080),
    };

    let result = generate_health_check_directive(&check).expect("Should generate successfully");
    assert_eq!(
        result,
        "health_check interval=10s fails=3 passes=2 uri=/health match=server_ok port=8080;"
    );
}

#[test]
fn test_invalid_health_check_uri() {
    let check = ActiveHealthCheck {
        interval: None,
        passes: None,
        fails: None,
        uri: Some("/health; drop table".to_string()),
        match_name: None,
        port: None,
    };

    let err = generate_health_check_directive(&check).unwrap_err();
    assert!(matches!(err, DomainError::InvalidInput(_)));
}

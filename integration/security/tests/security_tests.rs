#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use security::{
    audit::AuditRecord,
    firewall::{FirewallRule, RuleAction},
    router,
};
use std::net::SocketAddr;
use tower::ServiceExt;

#[tokio::test]
async fn test_rate_limit_exhaustion() {
    let app = router();
    let addr: SocketAddr = "127.0.0.1:8080".parse().unwrap();

    // The rate limiter is configured with a capacity of 10
    // So the first 10 requests should succeed (or return a route-specific status like 405 if empty method)
    // To properly test the middleware, we should hit an existing route.

    // We can simulate a lot of requests to hit the limit
    for _ in 0..10 {
        let req = Request::builder()
            .uri("/v1/security/firewall")
            .extension(axum::extract::ConnectInfo(addr))
            .body(Body::empty())
            .unwrap();

        let response = app.clone().oneshot(req).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // The 11th request should fail with TOO_MANY_REQUESTS
    let req = Request::builder()
        .uri("/v1/security/firewall")
        .extension(axum::extract::ConnectInfo(addr))
        .body(Body::empty())
        .unwrap();

    let response = app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_cidr_matching() {
    let firewall = security::firewall::Firewall::new();

    // Allow an IP
    firewall
        .add_rule(FirewallRule {
            cidr: "192.168.1.0/24".to_string(),
            action: RuleAction::Allow,
        })
        .await
        .unwrap();

    // Deny an IP
    firewall
        .add_rule(FirewallRule {
            cidr: "10.0.0.0/8".to_string(),
            action: RuleAction::Deny,
        })
        .await
        .unwrap();

    assert_eq!(
        firewall.check_ip("192.168.1.50".parse().unwrap()).await,
        RuleAction::Allow
    );
    assert_eq!(
        firewall.check_ip("10.5.5.5".parse().unwrap()).await,
        RuleAction::Deny
    );
    assert_eq!(
        firewall.check_ip("8.8.8.8".parse().unwrap()).await,
        RuleAction::Allow
    ); // Default is Allow
}

#[tokio::test]
async fn test_audit_record_structure() {
    let log = security::audit::AuditLog::new();
    let now = chrono::Utc::now();
    let record = AuditRecord {
        timestamp: now,
        action: "TEST_ACTION".to_string(),
        ip_address: Some("127.0.0.1".to_string()),
        resource: Some("/test".to_string()),
        success: true,
    };

    log.log(record.clone()).await;
    let records = log.get_records().await;

    assert_eq!(records.len(), 1);
    assert_eq!(records[0].action, "TEST_ACTION");
    assert_eq!(records[0].ip_address, Some("127.0.0.1".to_string()));
    assert_eq!(records[0].resource, Some("/test".to_string()));
    assert!(records[0].success);
}

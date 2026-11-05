#![forbid(unsafe_code)]

use tokio;
use reqwest;
use serde_json::json;

#[tokio::test]
async fn test_audit_logging_immutability() {
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let client = reqwest::Client::new();
    let res = client.get(&format!("{}/health", api_url)).send().await;

    if res.is_err() {
        panic!("BLOCKED: Missing real target running at {} for audit logging E2E verification. Failed to connect.", api_url);
    }

    let auth_token = std::env::var("FUSIONPANEL_TOKEN").unwrap_or_default();
    if auth_token.is_empty() {
        panic!("BLOCKED: Missing FUSIONPANEL_TOKEN for authenticated API calls.");
    }

    // Produce an audit event
    let event_res = client.post(&format!("{}/api/v1/audit", api_url))
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&json!({
            "action": "test_action",
            "resource": "test_resource"
        }))
        .send().await.expect("Failed to send request");

    if event_res.status() == reqwest::StatusCode::NOT_IMPLEMENTED {
        panic!("BLOCKED: Endpoint not implemented");
    }

    assert!(event_res.status().is_success(), "Failed to create audit event");

    // Instead of using systemctl to mutate the workstation service (forbidden),
    // block since a disposable target and isolated process execution capabilities are missing.
    panic!("BLOCKED: Missing disposable target to restart server for audit survival check.");
}

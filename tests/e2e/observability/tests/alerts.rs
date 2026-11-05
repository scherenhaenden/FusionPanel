#![forbid(unsafe_code)]

use tokio;
use reqwest;
use serde_json::json;

#[tokio::test]
async fn test_monitoring_alerts() {
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let client = reqwest::Client::new();
    let res = client.get(&format!("{}/health", api_url)).send().await;

    if res.is_err() {
        panic!("BLOCKED: Missing real target running at {} for monitoring alerts E2E verification. Failed to connect.", api_url);
    }

    let auth_token = std::env::var("FUSIONPANEL_TOKEN").unwrap_or_default();
    if auth_token.is_empty() {
        panic!("BLOCKED: Missing FUSIONPANEL_TOKEN for authenticated API calls.");
    }

    // Trigger an outage/alert condition
    let outage_res = client.post(&format!("{}/api/v1/debug/trigger-alert", api_url))
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&json!({
            "type": "outage"
        }))
        .send().await.expect("Failed to send request");

    if outage_res.status() == reqwest::StatusCode::NOT_IMPLEMENTED || outage_res.status() == reqwest::StatusCode::NOT_FOUND {
        panic!("BLOCKED: Endpoint not implemented or missing real target outage/repair mechanisms.");
    }

    assert!(outage_res.status().is_success(), "Failed to trigger alert");

    // Verify alert history
    let fetch_res = client.get(&format!("{}/api/v1/alerts/history", api_url))
        .header("Authorization", format!("Bearer {}", auth_token))
        .send().await.expect("Failed to send request");

    assert!(fetch_res.status().is_success(), "Failed to fetch alert history");
    let body = fetch_res.text().await.unwrap();
    assert!(body.contains("outage"), "Alert history did not contain expected outage");
}

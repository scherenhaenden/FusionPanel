#![forbid(unsafe_code)]

use tokio;
use reqwest;
use serde_json::json;

#[tokio::test]
async fn test_team_member_invitation_flow() {
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let client = reqwest::Client::new();
    let res = client.get(&format!("{}/health", api_url)).send().await;

    if res.is_err() {
        panic!("BLOCKED: Missing real target running at {} for invitation flow E2E verification. Failed to connect.", api_url);
    }

    let auth_token = std::env::var("FUSIONPANEL_TOKEN").unwrap_or_default();
    if auth_token.is_empty() {
        panic!("BLOCKED: Missing FUSIONPANEL_TOKEN for authenticated API calls.");
    }

    // Create an invitation
    let invite_res = client.post(&format!("{}/api/v1/invitations", api_url))
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&json!({
            "email": "test@example.com",
            "role": "member"
        }))
        .send().await.expect("Failed to send request");

    if invite_res.status() == reqwest::StatusCode::NOT_IMPLEMENTED {
        panic!("BLOCKED: Endpoint not implemented");
    }

    assert!(invite_res.status().is_success(), "Failed to create invitation");

    let invite_id: String = invite_res.json::<serde_json::Value>().await.unwrap()["id"].as_str().unwrap().to_string();

    // Revoke the invitation
    let revoke_res = client.delete(&format!("{}/api/v1/invitations/{}", api_url, invite_id))
        .header("Authorization", format!("Bearer {}", auth_token))
        .send().await.expect("Failed to send request");

    assert!(revoke_res.status().is_success(), "Failed to revoke invitation");

    // Attempt to accept revoked invitation
    let accept_res = client.post(&format!("{}/api/v1/invitations/{}/accept", api_url, invite_id))
        .send().await.expect("Failed to send request");

    assert_eq!(accept_res.status(), reqwest::StatusCode::FORBIDDEN, "Revoked user did not lose access");
}

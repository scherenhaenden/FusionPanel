#![forbid(unsafe_code)]

mod common;

use common::{app_state, RunningServer, ADMIN_TOKEN};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn invitation_acceptance_and_revocation_survive_service_restarts() {
    let temp = tempfile::tempdir().unwrap();
    let audit_path = temp.path().join("audit.jsonl");
    let invitations_path = temp.path().join("invitations.jsonl");
    let client = reqwest::Client::new();

    let first = RunningServer::start(integration_administration::routes::router(app_state(
        &audit_path,
        &invitations_path,
    )))
    .await;
    let created: serde_json::Value = client
        .post(format!("{}/invitations", first.base_url))
        .bearer_auth(ADMIN_TOKEN)
        .json(&json!({
            "tenant_id": Uuid::new_v4(),
            "role": "Admin",
            "ttl_seconds": 3600
        }))
        .send()
        .await
        .unwrap()
        .error_for_status()
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap();
    let token = created["token"].as_str().unwrap();
    first.stop().await;

    let after_restart = RunningServer::start(integration_administration::routes::router(
        app_state(&audit_path, &invitations_path),
    ))
    .await;
    let accepted = client
        .post(format!(
            "{}/invitations/{id}/accept",
            after_restart.base_url
        ))
        .bearer_auth(ADMIN_TOKEN)
        .json(&json!({"token": token}))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted.status(), reqwest::StatusCode::OK);

    let accepted_twice = client
        .post(format!(
            "{}/invitations/{id}/accept",
            after_restart.base_url
        ))
        .bearer_auth(ADMIN_TOKEN)
        .json(&json!({"token": token}))
        .send()
        .await
        .unwrap();
    assert_eq!(accepted_twice.status(), reqwest::StatusCode::CONFLICT);

    let revoked = client
        .post(format!(
            "{}/invitations/{id}/revoke",
            after_restart.base_url
        ))
        .bearer_auth(ADMIN_TOKEN)
        .send()
        .await
        .unwrap();
    assert_eq!(revoked.status(), reqwest::StatusCode::OK);
    after_restart.stop().await;

    let final_server = RunningServer::start(integration_administration::routes::router(app_state(
        &audit_path,
        &invitations_path,
    )))
    .await;
    let accepted_after_revoke = client
        .post(format!("{}/invitations/{id}/accept", final_server.base_url))
        .bearer_auth(ADMIN_TOKEN)
        .json(&json!({"token": token}))
        .send()
        .await
        .unwrap();
    assert_eq!(
        accepted_after_revoke.status(),
        reqwest::StatusCode::NOT_FOUND
    );
    final_server.stop().await;
}

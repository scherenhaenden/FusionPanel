#![forbid(unsafe_code)]

mod common;

use chrono::Utc;
use common::{app_state, RunningServer, ADMIN_TOKEN};
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn audit_log_is_authenticated_persistent_and_detects_tampering_after_restart() {
    let temp = tempfile::tempdir().unwrap();
    let audit_path = temp.path().join("audit.jsonl");
    let invitations_path = temp.path().join("invitations.jsonl");
    let client = reqwest::Client::new();

    let first_server = RunningServer::start(integration_administration::routes::router(app_state(
        &audit_path,
        &invitations_path,
    )))
    .await;
    let event = json!({
        "id": Uuid::new_v4(),
        "timestamp": Utc::now(),
        "actor": "e2e-admin",
        "action": "CREATE",
        "target": "resource-1",
        "tenant": "tenant-e2e",
        "ip_address": "127.0.0.1",
        "metadata": {"run": "audit-immutability"}
    });

    let unauthenticated = client
        .post(format!("{}/audit/append", first_server.base_url))
        .json(&event)
        .send()
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), reqwest::StatusCode::UNAUTHORIZED);

    let appended = client
        .post(format!("{}/audit/append", first_server.base_url))
        .bearer_auth(ADMIN_TOKEN)
        .json(&event)
        .send()
        .await
        .unwrap();
    assert_eq!(appended.status(), reqwest::StatusCode::OK);

    let verified: Value = client
        .get(format!("{}/audit/verify", first_server.base_url))
        .bearer_auth(ADMIN_TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(verified["valid"], true);
    first_server.stop().await;

    // Corrupt a durable field while leaving the stored hash intact.
    let stored = std::fs::read_to_string(&audit_path).unwrap();
    let corrupted = stored.replace("e2e-admin", "tampered-admin");
    assert_ne!(
        stored, corrupted,
        "fixture must contain the field under test"
    );
    std::fs::write(&audit_path, corrupted).unwrap();

    let restarted_state = app_state(&audit_path, &invitations_path);
    let restarted =
        RunningServer::start(integration_administration::routes::router(restarted_state)).await;
    let verification: Value = client
        .get(format!("{}/audit/verify", restarted.base_url))
        .bearer_auth(ADMIN_TOKEN)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(verification["valid"], false);
    restarted.stop().await;
}

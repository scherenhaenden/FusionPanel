#![forbid(unsafe_code)]

mod common;

use administration::quotas::{QuotaEngine, ResourceQuotas};
use common::{app_state, RunningServer, ADMIN_TOKEN};
use integration_administration::routes::router;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
async fn quota_routes_enforce_limits_under_concurrent_http_requests() {
    let temp = tempfile::tempdir().unwrap();
    let quotas = Arc::new(QuotaEngine::new());
    let tenant = Uuid::new_v4();
    quotas.set_limits(
        tenant,
        ResourceQuotas {
            cpu_cores: 4,
            memory_mb: 4096,
            disk_mb: 4096,
            domain_count: 4,
            database_count: 4,
        },
    );
    let mut state = app_state(
        &temp.path().join("audit.jsonl"),
        &temp.path().join("invitations.jsonl"),
    );
    state.quotas = quotas;
    let server = RunningServer::start(router(state)).await;
    let client = reqwest::Client::new();

    let mut requests = Vec::new();
    for _ in 0..8 {
        let client = client.clone();
        let url = format!("{}/quotas/reserve", server.base_url);
        requests.push(tokio::spawn(async move {
            client
                .post(url)
                .bearer_auth(ADMIN_TOKEN)
                .json(&json!({
                    "tenant_id": tenant,
                    "resources": {
                        "cpu_cores": 1,
                        "memory_mb": 1024,
                        "disk_mb": 1024,
                        "domain_count": 1,
                        "database_count": 1
                    }
                }))
                .send()
                .await
                .unwrap()
                .status()
        }));
    }

    let mut succeeded = 0;
    let mut rejected = 0;
    for request in requests {
        match request.await.unwrap() {
            reqwest::StatusCode::OK => succeeded += 1,
            reqwest::StatusCode::PAYLOAD_TOO_LARGE => rejected += 1,
            status => panic!("unexpected quota response: {status}"),
        }
    }
    assert_eq!(succeeded, 4);
    assert_eq!(rejected, 4);

    let released = client
        .post(format!("{}/quotas/release", server.base_url))
        .bearer_auth(ADMIN_TOKEN)
        .json(&json!({
            "tenant_id": tenant,
            "resources": {
                "cpu_cores": 1,
                "memory_mb": 1024,
                "disk_mb": 1024,
                "domain_count": 1,
                "database_count": 1
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(released.status(), reqwest::StatusCode::OK);

    let admitted = client
        .post(format!("{}/quotas/reserve", server.base_url))
        .bearer_auth(ADMIN_TOKEN)
        .json(&json!({
            "tenant_id": tenant,
            "resources": {
                "cpu_cores": 1,
                "memory_mb": 1024,
                "disk_mb": 1024,
                "domain_count": 1,
                "database_count": 1
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(admitted.status(), reqwest::StatusCode::OK);
    server.stop().await;
}

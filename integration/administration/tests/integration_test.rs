#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode, header::AUTHORIZATION},
};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;
use administration::{
    quotas::{QuotaEngine, ResourceQuotas},
    audit::AuditLog,
};
use integration_administration::routes::{router, AppState, DurableInvitations};
use uuid::Uuid;
use chrono::Utc;
use serde_json::json;
use http_body_util::BodyExt;
use tempfile::NamedTempFile;

async fn setup_app() -> (axum::Router, Arc<QuotaEngine>) {
    let quotas = Arc::new(QuotaEngine::new());

    let audit_file = NamedTempFile::new().unwrap();
    let audit = Arc::new(Mutex::new(AuditLog::with_path(audit_file.path()).unwrap()));

    let inv_file = NamedTempFile::new().unwrap();
    let invitations = Arc::new(Mutex::new(DurableInvitations::new(inv_file.path().to_path_buf())));

    let operators = Arc::new(Mutex::new(vec![
        integration_administration::routes::Operator {
            id: Uuid::new_v4(),
            username: "admin".to_string(),
            role: "system_admin".to_string(),
            token: "op-system-admin-token".to_string(),
        },
        integration_administration::routes::Operator {
            id: Uuid::new_v4(),
            username: "viewer".to_string(),
            role: "viewer".to_string(),
            token: "op-viewer-token".to_string(),
        }
    ]));

    let state = AppState {
        quotas: quotas.clone(),
        audit,
        invitations,
        operators,
        admin_token: Arc::new("secret-admin-token".to_string()),
    };
    (router(state), quotas)
}

async fn post_json_auth(app: &axum::Router, uri: &str, body: serde_json::Value) -> axum::http::Response<axum::body::Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(uri)
                .header("Content-Type", "application/json")
                .header(AUTHORIZATION, "Bearer secret-admin-token")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn get_auth(app: &axum::Router, uri: &str) -> axum::http::Response<axum::body::Body> {
    app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(uri)
                .header(AUTHORIZATION, "Bearer secret-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn test_health() {
    let (app, _) = setup_app().await;

    let response = app
        .oneshot(Request::builder().uri("/health").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_quotas_races_and_release() {
    let (app, quotas) = setup_app().await;
    let tenant_id = Uuid::new_v4();

    quotas.set_limits(tenant_id, ResourceQuotas {
        cpu_cores: 10,
        memory_mb: 1024,
        disk_mb: 10000,
        domain_count: 5,
        database_count: 2,
    });

    let mut futures = vec![];
    for _ in 0..10 {
        let app = app.clone();
        futures.push(tokio::spawn(async move {
            post_json_auth(&app, "/quotas/reserve", json!({
                "tenant_id": tenant_id,
                "resources": {
                    "cpu_cores": 1,
                    "memory_mb": 100,
                    "disk_mb": 1000,
                    "domain_count": 0,
                    "database_count": 0
                }
            })).await
        }));
    }

    let results = futures::future::join_all(futures).await;

    let mut success_count = 0;
    for res in results {
        if res.unwrap().status() == StatusCode::OK {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 10);

    // 11th fails
    let res = post_json_auth(&app, "/quotas/reserve", json!({
        "tenant_id": tenant_id,
        "resources": {
            "cpu_cores": 1,
            "memory_mb": 100,
            "disk_mb": 1000,
            "domain_count": 0,
            "database_count": 0
        }
    })).await;
    assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);

    // Test release
    let res = post_json_auth(&app, "/quotas/release", json!({
        "tenant_id": tenant_id,
        "resources": {
            "cpu_cores": 2,
            "memory_mb": 200,
            "disk_mb": 2000,
            "domain_count": 0,
            "database_count": 0
        }
    })).await;
    assert_eq!(res.status(), StatusCode::OK);

    // Now we can reserve again
    let res = post_json_auth(&app, "/quotas/reserve", json!({
        "tenant_id": tenant_id,
        "resources": {
            "cpu_cores": 1,
            "memory_mb": 100,
            "disk_mb": 1000,
            "domain_count": 0,
            "database_count": 0
        }
    })).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_audit_anchoring() {
    let (app, _) = setup_app().await;

    let event = json!({
        "id": Uuid::new_v4(),
        "timestamp": Utc::now(),
        "actor": "user",
        "action": "CREATE",
        "target": "res",
        "tenant": "tenant_1",
        "ip_address": "127.0.0.1",
        "metadata": null
    });

    // Unauthenticated should fail
    let unauth_res = app.clone().oneshot(
        Request::builder()
            .method("POST")
            .uri("/audit/append")
            .header("Content-Type", "application/json")
            .body(Body::from(event.to_string()))
            .unwrap()
    ).await.unwrap();
    assert_eq!(unauth_res.status(), StatusCode::UNAUTHORIZED);

    let res = post_json_auth(&app, "/audit/append", event).await;
    assert_eq!(res.status(), StatusCode::OK);

    let res = get_auth(&app, "/audit/verify").await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let val: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(val["valid"], true);
}

#[tokio::test]
async fn test_cross_tenant_invitation_policy() {
    let (app, _) = setup_app().await;
    let tenant_id = Uuid::new_v4();

    let res = post_json_auth(&app, "/invitations", json!({
        "tenant_id": tenant_id,
        "role": "Admin",
        "ttl_seconds": 3600
    })).await;
    assert_eq!(res.status(), StatusCode::OK);

    let body = res.into_body().collect().await.unwrap().to_bytes();
    let invitation: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let id = invitation["id"].as_str().unwrap();
    let token = invitation["token"].as_str().unwrap();

    // Remove unused current_time
    let res = post_json_auth(&app, &format!("/invitations/{}/accept", id), json!({
        "token": token
    })).await;
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_invitation_revocation_durable_survives_reload() {
    let inv_file = NamedTempFile::new().unwrap();
    let path = inv_file.path().to_path_buf();
    let tenant_id = Uuid::new_v4();

    // 1. Create invitation
    let inv = administration::teams::invitation::Invitation::new(
        tenant_id,
        administration::teams::invitation::Role::Member,
        3600,
    );
    let inv_id = inv.id;

    {
        let mut store = DurableInvitations::new(path.clone());
        store.insert(inv_id, inv).unwrap();
        // Revoke
        assert!(store.remove(&inv_id).unwrap().is_some());
    }

    // 2. Re-open store from disk - revoked invitation must NOT resurrect!
    {
        let mut reloaded_store = DurableInvitations::new(path);
        assert!(reloaded_store.get_mut(&inv_id).is_none());
        assert!(reloaded_store.remove(&inv_id).unwrap().is_none());
    }
}

#[tokio::test]
async fn test_rbac_system_admin_only() {
    let (app, _) = setup_app().await;

    // Viewer should be forbidden
    let res = app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/users")
                .header(AUTHORIZATION, "Bearer op-viewer-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);

    // System admin should be allowed
    let res = app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/users")
                .header(AUTHORIZATION, "Bearer op-system-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_audit_query_and_invitations() {
    let (app, _) = setup_app().await;

    // test GET /api/v1/admin/audit
    let res = app.clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/v1/admin/audit")
                .header(AUTHORIZATION, "Bearer op-system-admin-token")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    // test POST /api/v1/admin/invitations
    let res = app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/admin/invitations")
                .header(AUTHORIZATION, "Bearer op-system-admin-token")
                .header("Content-Type", "application/json")
                .body(Body::from(json!({
                    "tenant_id": Uuid::new_v4(),
                    "role": "Admin",
                    "ttl_seconds": 3600
                }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

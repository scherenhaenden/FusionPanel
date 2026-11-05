#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use containers::docker::engine::DockerCliEngine;
use http_body_util::BodyExt;
use identity::{
    rbac::{Permission, RoleBinding, RoleType, Scope},
    types::{TenantId, UserId},
};
use integration_containers::{
    ports::PortAllocator,
    routes::{create_router, AppState, AuditLogger},
    store::{ContainerStore, SqliteContainerStore},
};
use integration_identity::AuthenticatedUser;
use rusqlite::Connection;
use std::sync::Arc;
use tower::ServiceExt;

#[derive(Default)]
struct MockAuditLogger;

#[async_trait::async_trait]
impl AuditLogger for MockAuditLogger {
    async fn log_event(&self, _tenant_id: &str, _action: &str, _container_id: &str) {}
}

fn get_mock_user(tenant_id: &str) -> AuthenticatedUser {
    let uid: UserId = UserId::new_v4();
    let tid: TenantId = uuid::Uuid::parse_str(tenant_id).unwrap().into();
    AuthenticatedUser {
        user_id: uid.clone(),
        bindings: vec![RoleBinding {
            user_id: uid,
            role: identity::rbac::Role {
                id: identity::types::RoleId::new_v4(),
                name: "TenantAdmin".to_string(),
                role_type: RoleType::TenantAdmin,
                permissions: vec![Permission::SiteDeploy].into_iter().collect(),
            },
            scope: Scope::Tenant { tenant_id: tid },
        }],
    }
}

async fn setup_app() -> (axum::Router, Arc<SqliteContainerStore>, Arc<PortAllocator>) {
    let engine = Arc::new(DockerCliEngine::new());
    let audit_logger = Arc::new(MockAuditLogger::default());
    let port_allocator = Arc::new(PortAllocator::new(20000, 30000));

    // SQLite connection in memory
    let store = Arc::new(SqliteContainerStore::new(Connection::open_in_memory().unwrap()).unwrap());

    let state = Arc::new(AppState {
        engine: engine.clone(),
        audit_logger: audit_logger.clone(),
        port_allocator: port_allocator.clone(),
        store: store.clone(),
    });

    (
        create_router(state).expect("router setup should restore persisted port reservations"),
        store,
        port_allocator,
    )
}

#[tokio::test]
async fn test_docker_lifecycle_real_integration() {
    let (app, _store, _) = setup_app().await;

    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let user = get_mock_user(tenant_id);

    // 1. Create a container (Alpine sleep)
    let payload = serde_json::json!({
        "image": "alpine:latest",
        "command": ["sleep", "300"],
        "ports": []
    });

    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/v1/containers")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    if status != StatusCode::CREATED {
        println!("Error body: {:?}", String::from_utf8_lossy(&body_bytes));
    }
    if status == StatusCode::INTERNAL_SERVER_ERROR
        && String::from_utf8_lossy(&body_bytes).contains("failed to mount")
    {
        panic!("BLOCKED: Docker overlayfs restricted");
    }
    assert_eq!(status, StatusCode::CREATED);
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();

    let container_id = body["id"].as_str().unwrap().to_string();

    // 2. Start container
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/start", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 3. Inspect container
    let mut request = Request::builder()
        .method(Method::GET)
        .uri(format!("/v1/containers/{}", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["status"].as_str().unwrap(), "Running");

    // 4. Stop container
    let mut request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/stop", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    // 5. Remove container
    let mut request = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/v1/containers/{}", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());
    let response = app.clone().oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn test_store_reopen_and_reconcile() {
    // 1. Create store from a file to simulate restart persistence
    let db_path = "target/tmp_integration_reconcile.db";
    let _ = std::fs::remove_file(db_path);

    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let container_id = format!("{}-{}", tenant_id, uuid::Uuid::new_v4());

    {
        let store1 = SqliteContainerStore::new(Connection::open(db_path).unwrap()).unwrap();
        store1
            .create_container(&container_id, tenant_id, "test_app", &[24123])
            .unwrap();
    }

    // 2. Restart and reconcile
    {
        let store2 = SqliteContainerStore::new(Connection::open(db_path).unwrap()).unwrap();
        let records = store2.get_all_containers().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0, container_id);
        assert_eq!(records[0].1, tenant_id);

        // Reconcile: in a real system we would read records from the engine and synchronize the store.
        // For testing we just ensure the store re-opened correctly and data is there.
        let tenant = store2.get_container_tenant(&container_id).unwrap().unwrap();
        assert_eq!(tenant, tenant_id);
        assert_eq!(
            store2.get_container_ports(&container_id).unwrap(),
            vec![24123]
        );
        assert_eq!(store2.get_all_container_ports().unwrap(), vec![24123]);
    }

    let _ = std::fs::remove_file(db_path);
}

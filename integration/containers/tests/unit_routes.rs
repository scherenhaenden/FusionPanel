#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
};
use containers::{
    domain::{ContainerId, ContainerSpec, ContainerStatus},
    engine::ContainerEngine,
};
use http_body_util::BodyExt;
use integration_containers::{
    ports::PortAllocator,
    routes::{create_router, AppState, AuditLogger},
    store::{ContainerStore, SqliteContainerStore},
};
use integration_identity::AuthenticatedUser;
use identity::{
    rbac::{Permission, RoleBinding, RoleType, Scope},
    types::{TenantId, UserId},
};
use rusqlite::Connection;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

#[derive(Default)]
struct MockContainerEngine {
    create_calls: Arc<Mutex<Vec<ContainerSpec>>>,
    start_calls: Arc<Mutex<Vec<ContainerId>>>,
    stop_calls: Arc<Mutex<Vec<ContainerId>>>,
    remove_calls: Arc<Mutex<Vec<ContainerId>>>,
    inspect_calls: Arc<Mutex<Vec<ContainerId>>>,
    status_to_return: Arc<Mutex<Option<ContainerStatus>>>,
}

#[async_trait::async_trait]
impl ContainerEngine for MockContainerEngine {
    async fn create(&self, spec: &ContainerSpec) -> Result<(), containers::domain::DomainError> {
        self.create_calls.lock().unwrap().push(spec.clone());
        Ok(())
    }

    async fn start(&self, id: &ContainerId) -> Result<(), containers::domain::DomainError> {
        self.start_calls.lock().unwrap().push(id.clone());
        Ok(())
    }

    async fn stop(&self, id: &ContainerId) -> Result<(), containers::domain::DomainError> {
        self.stop_calls.lock().unwrap().push(id.clone());
        Ok(())
    }

    async fn remove(&self, id: &ContainerId) -> Result<(), containers::domain::DomainError> {
        self.remove_calls.lock().unwrap().push(id.clone());
        Ok(())
    }

    async fn inspect(
        &self,
        id: &ContainerId,
    ) -> Result<ContainerStatus, containers::domain::DomainError> {
        self.inspect_calls.lock().unwrap().push(id.clone());
        Ok(self.status_to_return.lock().unwrap().clone().unwrap_or(ContainerStatus::Created))
    }
}

#[derive(Default)]
struct MockAuditLogger {
    events: Arc<Mutex<Vec<(String, String, String)>>>,
}

#[async_trait::async_trait]
impl AuditLogger for MockAuditLogger {
    async fn log_event(&self, tenant_id: &str, action: &str, container_id: &str) {
        self.events.lock().unwrap().push((
            tenant_id.to_string(),
            action.to_string(),
            container_id.to_string(),
        ));
    }
}

fn setup() -> (axum::Router, Arc<MockContainerEngine>, Arc<MockAuditLogger>, Arc<SqliteContainerStore>) {
    let engine = Arc::new(MockContainerEngine::default());
    let audit_logger = Arc::new(MockAuditLogger::default());
    let port_allocator = Arc::new(PortAllocator::new(20000, 30000));
    let store = Arc::new(SqliteContainerStore::new(Connection::open_in_memory().unwrap()).unwrap());

    let state = Arc::new(AppState {
        engine: engine.clone(),
        audit_logger: audit_logger.clone(),
        port_allocator,
        store: store.clone(),
    });

    (create_router(state), engine, audit_logger, store)
}

fn get_mock_user(tenant_id: &str) -> AuthenticatedUser {
    let uid: UserId = UserId::new_v4();
    let tid: TenantId = uuid::Uuid::parse_str(tenant_id).unwrap().into();
    AuthenticatedUser {
        user_id: uid.clone(),
        bindings: vec![RoleBinding {
            user_id: uid,
            role: identity::rbac::Role {
                role_type: RoleType::TenantAdmin,
                id: identity::types::RoleId::new_v4(),
            name: "TenantAdmin".to_string(),
            permissions: vec![Permission::SiteDeploy].into_iter().collect(),
            },
            scope: Scope::Tenant { tenant_id: tid },
        }],
    }
}

#[tokio::test]
async fn test_create_container() {
    let (app, engine, audit_logger, store) = setup();

    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let user = get_mock_user(tenant_id);

    let payload = serde_json::json!({
        "image": "nginx:latest",
        "ports": [
            { "container": 80 }
        ],
        "mounts": [
            { "source": "/data", "target": "/app/data", "read_only": true }
        ]
    });

    let mut request = Request::builder()
        .method(Method::POST)
        .uri("/v1/containers")
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let container_id = body["id"].as_str().unwrap();

    assert!(container_id.starts_with(&format!("{}-", tenant_id)));

    // Verify ownership was persisted
    let stored_tenant = store.get_container_tenant(container_id).unwrap().unwrap();
    assert_eq!(stored_tenant, tenant_id);

    let create_calls = engine.create_calls.lock().unwrap();
    assert_eq!(create_calls.len(), 1);
    assert_eq!(create_calls[0].id.as_str(), container_id);
    assert_eq!(create_calls[0].ports.len(), 1);

    let events = audit_logger.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        (
            tenant_id.to_string(),
            "create".to_string(),
            container_id.to_string()
        )
    );
}

#[tokio::test]
async fn test_create_container_no_identity() {
    let (app, _engine, _audit_logger, _) = setup();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/containers")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"image": "nginx"}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    // Since we are mocking extractors inside the route we need to make sure the app handles missing auth
    // Wait, the Extension<AuthenticatedUser> will fail with 500 if missing unless we wrap it.
    // In Axum, missing Extension returns 500. So we expect 500 here since we didn't add the middleware.
    // In real app, the auth middleware adds it, if missing it returns 401.
    // We can assert 500 for missing extension.
    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
}

#[tokio::test]
async fn test_start_container_success() {
    let (app, engine, audit_logger, store) = setup();
    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let user = get_mock_user(tenant_id);
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    // Insert ownership
    store.create_container(&container_id, tenant_id, "app").unwrap();

    let mut request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/start", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let start_calls = engine.start_calls.lock().unwrap();
    assert_eq!(start_calls.len(), 1);
    assert_eq!(start_calls[0].as_str(), container_id);

    let events = audit_logger.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        (
            tenant_id.to_string(),
            "start".to_string(),
            container_id.clone()
        )
    );
}

#[tokio::test]
async fn test_start_container_wrong_tenant() {
    let (app, _engine, _audit_logger, store) = setup();
    let owner_tenant = "00000000-0000-0000-0000-000000000001";
    let attacker_tenant = "00000000-0000-0000-0000-000000000002";
    let user = get_mock_user(attacker_tenant);
    let container_id = format!("{}-{}", owner_tenant, Uuid::new_v4());

    store.create_container(&container_id, owner_tenant, "app").unwrap();

    let mut request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/start", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.oneshot(request).await.unwrap();

    // The attacker prefix doesn't match the container ID prefix
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_stop_container() {
    let (app, engine, audit_logger, store) = setup();
    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let user = get_mock_user(tenant_id);
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    store.create_container(&container_id, tenant_id, "app").unwrap();

    let mut request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/stop", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let stop_calls = engine.stop_calls.lock().unwrap();
    assert_eq!(stop_calls.len(), 1);
    assert_eq!(stop_calls[0].as_str(), container_id);

    let events = audit_logger.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        (
            tenant_id.to_string(),
            "stop".to_string(),
            container_id.clone()
        )
    );
}

#[tokio::test]
async fn test_restart_container() {
    let (app, engine, audit_logger, store) = setup();
    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let user = get_mock_user(tenant_id);
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    store.create_container(&container_id, tenant_id, "app").unwrap();

    let mut request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/restart", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let stop_calls = engine.stop_calls.lock().unwrap();
    assert_eq!(stop_calls.len(), 1);
    assert_eq!(stop_calls[0].as_str(), container_id);

    let start_calls = engine.start_calls.lock().unwrap();
    assert_eq!(start_calls.len(), 1);
    assert_eq!(start_calls[0].as_str(), container_id);

    let events = audit_logger.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        (
            tenant_id.to_string(),
            "restart".to_string(),
            container_id.clone()
        )
    );
}

#[tokio::test]
async fn test_remove_container() {
    let (app, engine, audit_logger, store) = setup();
    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let user = get_mock_user(tenant_id);
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    store.create_container(&container_id, tenant_id, "app").unwrap();

    let mut request = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/v1/containers/{}", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let remove_calls = engine.remove_calls.lock().unwrap();
    assert_eq!(remove_calls.len(), 1);
    assert_eq!(remove_calls[0].as_str(), container_id);

    let db_record = store.get_container_tenant(&container_id).unwrap();
    assert!(db_record.is_none());

    let events = audit_logger.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        (
            tenant_id.to_string(),
            "remove".to_string(),
            container_id.clone()
        )
    );
}

#[tokio::test]
async fn test_inspect_container() {
    let (app, engine, audit_logger, store) = setup();
    let tenant_id = "00000000-0000-0000-0000-000000000001";
    let user = get_mock_user(tenant_id);
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    store.create_container(&container_id, tenant_id, "app").unwrap();

    *engine.status_to_return.lock().unwrap() = Some(ContainerStatus::Running);

    let mut request = Request::builder()
        .method(Method::GET)
        .uri(format!("/v1/containers/{}", container_id))
        .body(Body::empty())
        .unwrap();
    request.extensions_mut().insert(user.clone());

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["status"].as_str().unwrap(), "Running");

    let inspect_calls = engine.inspect_calls.lock().unwrap();
    assert_eq!(inspect_calls.len(), 1);
    assert_eq!(inspect_calls[0].as_str(), container_id);

    let events = audit_logger.events.lock().unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0],
        (
            tenant_id.to_string(),
            "inspect".to_string(),
            container_id.clone()
        )
    );
}

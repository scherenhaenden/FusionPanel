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
use std::sync::{Arc, Mutex};
use tower::ServiceExt;
use uuid::Uuid;

use crate::routes::{create_router, AppState, AuditLogger};

struct MockContainerEngine {
    create_calls: Arc<Mutex<Vec<ContainerSpec>>>,
    start_calls: Arc<Mutex<Vec<ContainerId>>>,
    stop_calls: Arc<Mutex<Vec<ContainerId>>>,
    remove_calls: Arc<Mutex<Vec<ContainerId>>>,
    inspect_calls: Arc<Mutex<Vec<ContainerId>>>,
    status_to_return: Arc<Mutex<ContainerStatus>>,
}

impl Default for MockContainerEngine {
    fn default() -> Self {
        Self {
            create_calls: Default::default(),
            start_calls: Default::default(),
            stop_calls: Default::default(),
            remove_calls: Default::default(),
            inspect_calls: Default::default(),
            status_to_return: Arc::new(Mutex::new(ContainerStatus::Created)),
        }
    }
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
        Ok(*self.status_to_return.lock().unwrap())
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

fn setup() -> (axum::Router, Arc<MockContainerEngine>, Arc<MockAuditLogger>) {
    let engine = Arc::new(MockContainerEngine::default());
    let audit_logger = Arc::new(MockAuditLogger::default());

    let state = Arc::new(AppState {
        engine: engine.clone(),
        audit_logger: audit_logger.clone(),
    });

    (create_router(state), engine, audit_logger)
}

#[tokio::test]
async fn test_create_container() {
    let (app, engine, audit_logger) = setup();

    let tenant_id = "tenant-1";
    let payload = serde_json::json!({
        "image": "nginx:latest",
        "ports": [
            { "host": 8080, "container": 80 }
        ],
        "mounts": [
            { "source": "/data", "target": "/app/data", "read_only": true }
        ]
    });

    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/containers")
        .header("x-tenant-id", tenant_id)
        .header("content-type", "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let container_id = body["id"].as_str().unwrap();

    assert!(container_id.starts_with(&format!("{}-", tenant_id)));

    let create_calls = engine.create_calls.lock().unwrap();
    assert_eq!(create_calls.len(), 1);
    assert_eq!(create_calls[0].id.as_str(), container_id);

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
async fn test_create_container_no_tenant() {
    let (app, _engine, _audit_logger) = setup();

    let request = Request::builder()
        .method(Method::POST)
        .uri("/v1/containers")
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::json!({"image": "nginx"}).to_string(),
        ))
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_start_container_success() {
    let (app, engine, audit_logger) = setup();
    let tenant_id = "tenant-1";
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/start", container_id))
        .header("x-tenant-id", tenant_id)
        .body(Body::empty())
        .unwrap();

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
    let (app, _engine, _audit_logger) = setup();
    let container_id = format!("tenant-1-{}", Uuid::new_v4());

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/start", container_id))
        .header("x-tenant-id", "tenant-2")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_stop_container() {
    let (app, engine, audit_logger) = setup();
    let tenant_id = "tenant-1";
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/stop", container_id))
        .header("x-tenant-id", tenant_id)
        .body(Body::empty())
        .unwrap();

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
    let (app, engine, audit_logger) = setup();
    let tenant_id = "tenant-1";
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/restart", container_id))
        .header("x-tenant-id", tenant_id)
        .body(Body::empty())
        .unwrap();

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
    let (app, engine, audit_logger) = setup();
    let tenant_id = "tenant-1";
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    let request = Request::builder()
        .method(Method::DELETE)
        .uri(format!("/v1/containers/{}", container_id))
        .header("x-tenant-id", tenant_id)
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);

    let remove_calls = engine.remove_calls.lock().unwrap();
    assert_eq!(remove_calls.len(), 1);
    assert_eq!(remove_calls[0].as_str(), container_id);

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
    let (app, engine, audit_logger) = setup();
    let tenant_id = "tenant-1";
    let container_id = format!("{}-{}", tenant_id, Uuid::new_v4());

    *engine.status_to_return.lock().unwrap() = ContainerStatus::Running;

    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/v1/containers/{}", container_id))
        .header("x-tenant-id", tenant_id)
        .body(Body::empty())
        .unwrap();

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

#[tokio::test]
async fn test_invalid_container_id_format() {
    let (app, _engine, _audit_logger) = setup();
    let tenant_id = "tenant-1";
    let container_id = format!("{}-!invalid", tenant_id); // Invalid chars

    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/v1/containers/{}/start", container_id))
        .header("x-tenant-id", tenant_id)
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

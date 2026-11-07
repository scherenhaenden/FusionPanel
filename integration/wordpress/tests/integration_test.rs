#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use integration_wordpress::{router, AppState, WordPressInstance};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_get_wordpress_instance_integration() {
    let app_state = AppState {
        tenant_id: Arc::new("tenant-456".to_string()),
        jobs: Arc::new(Mutex::new(Vec::new())),
        instances: Arc::new(Mutex::new(vec![])),
    };

    let id = Uuid::new_v4();
    app_state.instances.lock().await.push(WordPressInstance {
        id,
        domain: "example.com".to_string(),
        version: "6.4.3".to_string(),
        tenant_id: "tenant-456".to_string(),
    });

    let app = Router::new().nest("/v1/wordpress", router(app_state));

    let req = Request::builder()
        .method("GET")
        .uri(format!("/v1/wordpress/{}", id))
        .header("X-Tenant-ID", "tenant-456")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_upgrade_wordpress_integration() {
    let app_state = AppState {
        tenant_id: Arc::new("tenant-456".to_string()),
        jobs: Arc::new(Mutex::new(Vec::new())),
        instances: Arc::new(Mutex::new(vec![])),
    };

    let id = Uuid::new_v4();
    app_state.instances.lock().await.push(WordPressInstance {
        id,
        domain: "example.com".to_string(),
        version: "6.4.3".to_string(),
        tenant_id: "tenant-456".to_string(),
    });

    let app = Router::new().nest("/v1/wordpress", router(app_state));

    let req = Request::builder()
        .method("POST")
        .uri(format!("/v1/wordpress/{}/upgrade", id))
        .header("X-Tenant-ID", "tenant-456")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"version":"6.5.0"}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_plugins_wordpress_integration() {
    let app_state = AppState {
        tenant_id: Arc::new("tenant-456".to_string()),
        jobs: Arc::new(Mutex::new(Vec::new())),
        instances: Arc::new(Mutex::new(vec![])),
    };

    let id = Uuid::new_v4();
    app_state.instances.lock().await.push(WordPressInstance {
        id,
        domain: "example.com".to_string(),
        version: "6.4.3".to_string(),
        tenant_id: "tenant-456".to_string(),
    });

    let app = Router::new().nest("/v1/wordpress", router(app_state));

    let req = Request::builder()
        .method("POST")
        .uri(format!("/v1/wordpress/{}/plugins", id))
        .header("X-Tenant-ID", "tenant-456")
        .header("Content-Type", "application/json")
        .body(Body::from(r#"{"plugins":["woocommerce","yoast-seo"]}"#))
        .unwrap();

    let response = app.oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

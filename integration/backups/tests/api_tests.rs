#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backups_integration::models::{BackupConfig, StorageDestination};
use backups_integration::routes::{router, AppState};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tower::ServiceExt;

#[tokio::test]
async fn test_create_and_get_config() {
    let state = AppState {
        configs: Arc::new(Mutex::new(HashMap::new())),
        outcomes: Arc::new(Mutex::new(HashMap::new())),
    };

    let app = router(state);

    let config = BackupConfig {
        id: "".to_string(),
        schedule: "0 0 0 * * * *".to_string(),
        databases: vec!["db1".to_string()],
        files: vec![],
        retention_count: 5,
        storage_destinations: vec![StorageDestination::Local {
            path: "/tmp".into(),
        }],
        encryption_key_ref: "key-123".to_string(),
    };

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/backups/configs")
                .header("Content-Type", "application/json")
                .body(Body::from(serde_json::to_vec(&config).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let created_config: BackupConfig = serde_json::from_slice(&body).unwrap();

    assert!(!created_config.id.is_empty());

    // Get
    let response = app
        .oneshot(
            Request::builder()
                .uri(format!("/backups/configs/{}", created_config.id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_trigger_backup() {
    let state = AppState {
        configs: Arc::new(Mutex::new(HashMap::new())),
        outcomes: Arc::new(Mutex::new(HashMap::new())),
    };

    let config_id = "test-config".to_string();

    {
        let mut configs = state.configs.lock().unwrap();
        configs.insert(
            config_id.clone(),
            BackupConfig {
                id: config_id.clone(),
                schedule: "0 0 0 * * * *".to_string(),
                databases: vec!["db1".to_string()],
                files: vec![],
                retention_count: 5,
                storage_destinations: vec![],
                encryption_key_ref: "".to_string(),
            },
        );
    }

    let app = router(state);

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/backups/configs/{}/run", config_id))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
}

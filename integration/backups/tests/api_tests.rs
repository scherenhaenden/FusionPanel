#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use backups_integration::routes::{router, AppState};
use tower::ServiceExt;

#[tokio::test]
async fn backup_api_fails_closed_until_durable_store_and_worker_are_configured() {
    let app = router(AppState::default());

    for (method, uri) in [
        ("POST", "/backups/configs"),
        ("GET", "/backups/configs"),
        ("GET", "/backups/configs/test-config"),
        ("POST", "/backups/configs/test-config/run"),
        ("GET", "/backups/runs/test-run"),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(uri)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "{method} {uri}"
        );
        let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let body: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(body["error"].as_str().unwrap().contains("BLOCKED"));
    }
}

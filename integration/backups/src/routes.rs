#![forbid(unsafe_code)]

use axum::{
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde_json::json;

/// Placeholder state retained for API compatibility while the durable config
/// store and worker integration are not available.
#[derive(Clone, Default)]
pub struct AppState;

pub fn router(_state: AppState) -> Router {
    Router::new()
        .route("/backups/configs", post(blocked).get(blocked))
        .route("/backups/configs/:id", get(blocked))
        .route("/backups/configs/:id/run", post(blocked))
        .route("/backups/runs/:id", get(blocked))
}

async fn blocked() -> impl IntoResponse {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(json!({
            "error": "BLOCKED: durable backup configuration, job storage, and worker integration are not configured"
        })),
    )
}

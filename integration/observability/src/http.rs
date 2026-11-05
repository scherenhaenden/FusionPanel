#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use observability::{metrics::MetricsRegistry, store::IncidentStore};
use std::path::Path;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;

#[derive(Clone)]
pub struct AppState {
    pub metrics: Arc<MetricsRegistry>,
    pub store: Arc<Mutex<IncidentStore>>,
    admin_token: Arc<String>,
}

impl AppState {
    pub fn new(
        db_path: &Path,
        admin_token: impl Into<String>,
    ) -> Result<Self, Box<dyn std::error::Error>> {
        Ok(Self {
            metrics: Arc::new(MetricsRegistry::new()?),
            store: Arc::new(Mutex::new(IncidentStore::new(db_path)?)),
            admin_token: Arc::new(admin_token.into()),
        })
    }
}

fn auth_check(headers: &HeaderMap, expected_token: &str) -> bool {
    if expected_token.is_empty() {
        return false;
    }
    headers
        .get("authorization")
        .and_then(|value| value.to_str().ok())
        .map(|value| value == format!("Bearer {expected_token}"))
        .unwrap_or(false)
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "fusionpanel-api",
        "version": "0.1.0"
    }))
}

async fn capabilities(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if !auth_check(&headers, &state.admin_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {"code": "unauthorized", "message": "Authentication required"}
            })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "capabilities": ["control-plane.health.read", "control-plane.capabilities.read"]
        })),
    )
}

async fn get_metrics(State(state): State<AppState>) -> impl IntoResponse {
    use prometheus::Encoder;
    let mut buffer = vec![];
    let encoder = prometheus::TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let output = String::from_utf8(buffer).unwrap();
    (StatusCode::OK, output)
}

async fn get_incidents(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if !auth_check(&headers, &state.admin_token) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {"code": "unauthorized", "message": "Authentication required"}
            })),
        );
    }

    let store = state.store.lock().await;
    match store.get_all_incidents(None) {
        Ok(incidents) => (
            StatusCode::OK,
            Json(serde_json::json!({ "incidents": incidents })),
        ),
        Err(error) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {"code": "internal_error", "message": error.to_string()}
            })),
        ),
    }
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({"error": "not_found"})),
    )
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health_check))
        .route("/v1/capabilities", get(capabilities))
        .route("/metrics", get(get_metrics))
        .route("/incidents", get(get_incidents))
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

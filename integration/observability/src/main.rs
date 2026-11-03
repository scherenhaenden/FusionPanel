use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use observability::{metrics::MetricsRegistry, store::IncidentStore};
use std::env;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod collector;
use collector::{CollectorWorker, ProbeTarget};

#[derive(Clone)]
struct AppState {
    metrics: Arc<MetricsRegistry>,
    store: Arc<Mutex<IncidentStore>>,
}

fn auth_check(headers: &HeaderMap) -> bool {
    let expected_token = env::var("FUSIONPANEL_ADMIN_TOKEN").unwrap_or_else(|_| "".to_string());
    if expected_token.is_empty() {
        // As a safe default if not provided, deny all requests, though the contract expects it to be present
        return false;
    }
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(auth_str) = auth_header.to_str() {
            if auth_str == format!("Bearer {}", expected_token) {
                return true;
            }
        }
    }
    false
}

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({
        "status": "ok",
        "service": "fusionpanel-api",
        "version": "0.1.0"
    }))
}

async fn capabilities(headers: HeaderMap) -> impl IntoResponse {
    if !auth_check(&headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "code": "unauthorized",
                    "message": "Authentication required"
                }
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

async fn get_incidents(headers: HeaderMap, State(state): State<AppState>) -> impl IntoResponse {
    if !auth_check(&headers) {
        return (
            StatusCode::UNAUTHORIZED,
            Json(serde_json::json!({
                "error": {
                    "code": "unauthorized",
                    "message": "Authentication required"
                }
            })),
        );
    }

    let store = state.store.lock().await;
    // For now we pass None for tenant_id since we are using a global token.
    // In a real app we'd extract the tenant_id from the authenticated user.
    match store.get_all_incidents(None) {
        Ok(incidents) => (
            StatusCode::OK,
            Json(serde_json::json!({ "incidents": incidents })),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": {
                    "code": "internal_error",
                    "message": e.to_string()
                }
            })),
        ),
    }
}

async fn not_found() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(serde_json::json!({
            "error": "not_found"
        })),
    )
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let store = Arc::new(Mutex::new(
        IncidentStore::new(std::path::Path::new("incidents.db")).expect("Failed to initialize DB"),
    ));
    let metrics = Arc::new(MetricsRegistry::new().expect("Failed to create metrics registry"));

    let state = AppState {
        metrics: metrics.clone(),
        store: store.clone(),
    };

    // Load targets from env if present for E2E
    let target_url = env::var("TEST_TARGET_URL").unwrap_or_default();
    let webhook_url = env::var("TEST_WEBHOOK_URL").ok();

    let mut targets = Vec::new();
    if !target_url.is_empty() {
        targets.push(ProbeTarget {
            id: "target-1".to_string(),
            target_type: if target_url.starts_with("http") {
                "http".to_string()
            } else {
                "tcp".to_string()
            },
            url_or_addr: target_url,
        });
    }

    let worker = CollectorWorker::new(targets, metrics, store, webhook_url);
    tokio::spawn(async move {
        worker.run().await;
    });

    let app = Router::new()
        .route("/v1/health", get(health_check))
        .route("/v1/capabilities", get(capabilities))
        .route("/metrics", get(get_metrics))
        .route("/incidents", get(get_incidents))
        .fallback(not_found)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listen_addr =
        env::var("FUSIONPANEL_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&listen_addr).await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, app).await.unwrap();
}

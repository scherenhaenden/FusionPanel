use axum::{
    extract::{Request, State},
    http::{HeaderMap, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Json},
    routing::get,
    Router,
};
use serde_json::json;
use std::sync::Arc;

pub struct AppState {
    pub token: String,
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/health", get(health_handler))
        .route(
            "/v1/capabilities",
            get(capabilities_handler).route_layer(middleware::from_fn_with_state(
                state.clone(),
                auth_middleware,
            )),
        )
        .fallback(not_found_handler)
        .with_state(state)
}

async fn health_handler() -> impl IntoResponse {
    let version = env!("CARGO_PKG_VERSION");
    Json(json!({
        "status": "ok",
        "service": "fusionpanel-api",
        "version": version
    }))
}

async fn capabilities_handler() -> impl IntoResponse {
    Json(json!({
        "capabilities": [
            "control-plane.health.read",
            "control-plane.capabilities.read"
        ]
    }))
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    request: Request,
    next: Next,
) -> Result<impl IntoResponse, (StatusCode, Json<serde_json::Value>)> {
    let auth_header = headers.get("authorization").and_then(|h| h.to_str().ok());
    if let Some(auth_header) = auth_header {
        if auth_header.starts_with("Bearer ") {
            let token = auth_header.trim_start_matches("Bearer ");
            if token == state.token {
                return Ok(next.run(request).await);
            }
        }
    }

    Err((
        StatusCode::UNAUTHORIZED,
        Json(json!({
            "error": {
                "code": "unauthorized",
                "message": "Authentication required"
            }
        })),
    ))
}

async fn not_found_handler() -> impl IntoResponse {
    (
        StatusCode::NOT_FOUND,
        Json(json!({
            "error": {
                "code": "not_found",
                "message": "Not found"
            }
        })),
    )
}

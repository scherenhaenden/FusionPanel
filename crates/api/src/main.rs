#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::{header::AUTHORIZATION, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Serialize;
use std::{env, sync::Arc};
use subtle::ConstantTimeEq;
use tokio::net::TcpListener;
use tokio::signal;

#[derive(Clone)]
struct AppState {
    admin_token: Arc<String>,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    service: &'static str,
    version: &'static str,
}

#[derive(Serialize)]
struct CapabilitiesResponse {
    capabilities: Vec<&'static str>,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: &'static str,
    message: &'static str,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.error.code {
            "unauthorized" => StatusCode::UNAUTHORIZED,
            "not_found" => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        service: "fusionpanel-api",
        version: env!("CARGO_PKG_VERSION"),
    })
}

async fn capabilities_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<Json<CapabilitiesResponse>, ErrorResponse> {
    let auth_header = headers.get(AUTHORIZATION).and_then(|h| h.to_str().ok());

    let token_valid = if let Some(auth_str) = auth_header {
        if let Some(token) = auth_str.strip_prefix("Bearer ") {
            // Use subtle for constant time comparison to avoid timing attacks
            let provided = token.as_bytes();
            let expected = state.admin_token.as_bytes();
            if provided.len() == expected.len() {
                provided.ct_eq(expected).into()
            } else {
                false
            }
        } else {
            false
        }
    } else {
        false
    };

    if token_valid {
        Ok(Json(CapabilitiesResponse {
            capabilities: vec![
                "control-plane.health.read",
                "control-plane.capabilities.read",
            ],
        }))
    } else {
        Err(ErrorResponse {
            error: ErrorDetail {
                code: "unauthorized",
                message: "Authentication required",
            },
        })
    }
}

async fn fallback_handler() -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "not_found",
            message: "Not found",
        },
    }
}

fn app(state: AppState) -> Router {
    Router::new()
        .route("/v1/health", get(health_handler))
        .route("/v1/capabilities", get(capabilities_handler))
        .fallback(fallback_handler)
        .with_state(state)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let admin_token = match env::var("FUSIONPANEL_ADMIN_TOKEN") {
        Ok(token) if !token.is_empty() => token,
        _ => {
            eprintln!("FUSIONPANEL_ADMIN_TOKEN is required and must not be empty");
            std::process::exit(1);
        }
    };

    let listen_addr =
        env::var("FUSIONPANEL_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".to_string());

    let state = AppState {
        admin_token: Arc::new(admin_token),
    };

    let app = app(state);

    let listener = TcpListener::bind(&listen_addr).await?;
    println!("Listening on {}", listen_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    fn test_app() -> Router {
        app(AppState {
            admin_token: Arc::new("test-secret-token".to_string()),
        })
    }

    #[tokio::test]
    async fn test_health() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            format!(r#"{{"status":"ok","service":"fusionpanel-api","version":"{}"}}"#, env!("CARGO_PKG_VERSION"))
        );
    }

    #[tokio::test]
    async fn test_capabilities_no_auth() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            r#"{"error":{"code":"unauthorized","message":"Authentication required"}}"#
        );
    }

    #[tokio::test]
    async fn test_capabilities_valid_auth() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .header("Authorization", "Bearer test-secret-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            r#"{"capabilities":["control-plane.health.read","control-plane.capabilities.read"]}"#
        );
    }

    #[tokio::test]
    async fn test_capabilities_invalid_auth() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/capabilities")
                    .header("Authorization", "Bearer wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_not_found() {
        let app = test_app();

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/v1/unknown")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();
        assert_eq!(
            body_str,
            r#"{"error":{"code":"not_found","message":"Not found"}}"#
        );
    }
}

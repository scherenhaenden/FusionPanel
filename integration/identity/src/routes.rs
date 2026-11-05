#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use identity::{password::PasswordManager, session::Session, types::TenantId, SqliteIdentityStore};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_cookies::{Cookie, Cookies};

#[derive(Clone)]
pub struct AuthState {
    pub store: Arc<Mutex<SqliteIdentityStore>>,
}

#[derive(Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::UNAUTHORIZED, Json(self)).into_response()
    }
}

pub fn auth_routes(state: AuthState) -> Router {
    Router::new()
        .route("/api/v1/auth/login", post(login_handler))
        .route("/api/v1/auth/refresh", post(refresh_handler))
        .route("/api/v1/auth/logout", post(logout_handler))
        .with_state(state)
}

async fn login_handler(
    State(state): State<AuthState>,
    cookies: Cookies,
    Json(payload): Json<LoginRequest>,
) -> Result<StatusCode, ErrorResponse> {
    // RATE-LIMITING HOOK: Integrate external rate limiting check here.
    // E.g., rate_limiter.check(&payload.username).await?;

    let store = state.store.lock().await;

    let user = match store.get_user_by_username(&payload.username) {
        Ok(Some(u)) => u,
        _ => {
            println!(
                "AUDIT EVENT: Failed login attempt for username: {}",
                payload.username
            );
            return Err(ErrorResponse {
                error: "Invalid credentials".into(),
            });
        }
    };

    if let Err(_) = PasswordManager::verify_password(&payload.password, user.password_hash()) {
        println!(
            "AUDIT EVENT: Failed password verification for username: {}",
            payload.username
        );
        return Err(ErrorResponse {
            error: "Invalid credentials".into(),
        });
    }

    let tenant_id = TenantId::new_v4(); // Placeholder
    let session = Session::new(user.id(), tenant_id, 86400, 3600).map_err(|_| ErrorResponse {
        error: "Internal error creating session".into(),
    })?;

    if let Err(_) = store.save_session(&session) {
        return Err(ErrorResponse {
            error: "Internal error saving session".into(),
        });
    }

    let mut cookie = Cookie::new("SESSION_ID", session.id().to_string());
    cookie.set_http_only(true);
    cookie.set_secure(true);
    cookie.set_same_site(tower_cookies::cookie::SameSite::Strict);
    cookie.set_path("/");

    cookies.add(cookie);

    Ok(StatusCode::OK)
}

async fn refresh_handler(
    State(state): State<AuthState>,
    cookies: Cookies,
) -> Result<StatusCode, ErrorResponse> {
    let session_id_str = cookies.get("SESSION_ID").map(|c| c.value().to_string());
    let session_id = match session_id_str {
        Some(s) => s.parse().map_err(|_| ErrorResponse {
            error: "Invalid session cookie format".into(),
        })?,
        None => {
            return Err(ErrorResponse {
                error: "No session cookie".into(),
            })
        }
    };

    let store = state.store.lock().await;

    let now = chrono::Utc::now();
    if let Err(_) = store.update_session_touch(session_id, now) {
        return Err(ErrorResponse {
            error: "Session expired or revoked".into(),
        });
    }

    Ok(StatusCode::OK)
}

async fn logout_handler(
    State(state): State<AuthState>,
    cookies: Cookies,
) -> Result<StatusCode, ErrorResponse> {
    let session_id_str = cookies.get("SESSION_ID").map(|c| c.value().to_string());

    if let Some(session_id_str) = session_id_str {
        if let Ok(session_id) = session_id_str.parse() {
            let store = state.store.lock().await;
            let _ = store.revoke_session(session_id, "User requested logout");
        }
    }

    let mut cookie = Cookie::build(("SESSION_ID", "")).path("/").build();
    cookie.make_removal();
    cookies.add(cookie);

    Ok(StatusCode::OK)
}
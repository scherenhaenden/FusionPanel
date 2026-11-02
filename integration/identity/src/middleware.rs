#![forbid(unsafe_code)]

use axum::{
    extract::{Request, State},
    http::{header, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use chrono::Utc;
use serde::Serialize;
use std::sync::Arc;

use identity::error::IdentityError;
use identity::session::Session;
use identity::types::SessionId;

#[async_trait::async_trait]
pub trait IdentityStore: Send + Sync + 'static {
    async fn get_session(&self, id: SessionId) -> Result<Option<Session>, IdentityError>;
    async fn update_session_touch(
        &self,
        id: SessionId,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), IdentityError>;
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

fn unauthorized(message: &str) -> Response {
    let body = Json(ErrorResponse {
        error: ErrorDetail {
            code: "unauthorized".to_string(),
            message: message.to_string(),
        },
    });
    (StatusCode::UNAUTHORIZED, body).into_response()
}

pub async fn auth_middleware<S: IdentityStore>(
    State(store): State<Arc<S>>,
    mut req: Request,
    next: Next,
) -> Response {
    let mut token = None;

    if let Some(auth_header) = req.headers().get(header::AUTHORIZATION) {
        if let Ok(auth_str) = auth_header.to_str() {
            if let Some(stripped) = auth_str.strip_prefix("Bearer ") {
                token = Some(stripped.to_string());
            }
        }
    }

    if token.is_none() {
        if let Some(cookie_header) = req.headers().get(header::COOKIE) {
            if let Ok(cookie_str) = cookie_header.to_str() {
                for cookie in cookie_str.split(';') {
                    let cookie = cookie.trim();
                    if let Some(stripped) = cookie.strip_prefix("session_id=") {
                        token = Some(stripped.to_string());
                        break;
                    }
                }
            }
        }
    }

    let session_id_str = match token {
        Some(t) => t,
        None => return unauthorized("Authentication required"),
    };

    let session_id = match session_id_str.parse::<SessionId>() {
        Ok(id) => id,
        Err(_) => return unauthorized("Authentication required"),
    };

    let mut session = match store.get_session(session_id).await {
        Ok(Some(s)) => s,
        Ok(None) => return unauthorized("Authentication required"),
        Err(_) => return unauthorized("Authentication required"),
    };

    let now = Utc::now();
    if session.validate_and_touch(now).is_err() {
        return unauthorized("Authentication required");
    }

    if store.update_session_touch(session_id, now).await.is_err() {
        return unauthorized("Authentication required");
    }

    req.extensions_mut().insert(session);
    next.run(req).await
}

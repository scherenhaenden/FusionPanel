#![forbid(unsafe_code)]

use axum::{
    body::Body,
    http::{Request, StatusCode},
    middleware::from_fn_with_state,
    response::IntoResponse,
    routing::get,
    Router,
};
use chrono::Utc;
use http_body_util::BodyExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower::ServiceExt;

use identity::{
    error::IdentityError,
    session::Session,
    types::{SessionId, TenantId, UserId},
};
use integration_identity::{auth_middleware, IdentityStore};

#[derive(Clone, Default)]
struct MockStore {
    sessions: Arc<Mutex<Vec<Session>>>,
    update_fail: bool,
    get_fail: bool,
}

#[async_trait::async_trait]
impl IdentityStore for MockStore {
    async fn get_session(&self, id: SessionId) -> Result<Option<Session>, IdentityError> {
        if self.get_fail {
            return Err(IdentityError::OperationFailed {
                message: "get_fail".to_string(),
            });
        }
        let sessions = self.sessions.lock().await;
        Ok(sessions.iter().find(|s| s.id() == id).cloned())
    }

    async fn update_session_touch(
        &self,
        id: SessionId,
        now: chrono::DateTime<Utc>,
    ) -> Result<(), IdentityError> {
        if self.update_fail {
            return Err(IdentityError::OperationFailed {
                message: "update_fail".to_string(),
            });
        }
        let mut sessions = self.sessions.lock().await;
        if let Some(session) = sessions.iter_mut().find(|s| s.id() == id) {
            let _ = session.validate_and_touch(now);
        }
        Ok(())
    }

    async fn get_user_role_bindings(
        &self,
        _user_id: UserId,
    ) -> Result<Vec<identity::rbac::RoleBinding>, IdentityError> {
        Ok(vec![])
    }

    async fn is_revoked(&self, session: &Session) -> Result<bool, IdentityError> {
        Ok(session.is_revoked())
    }
}

async fn protected_handler() -> impl IntoResponse {
    (StatusCode::OK, "protected")
}

fn create_app(store: MockStore) -> Router {
    let state = Arc::new(store);
    Router::new()
        .route("/", get(protected_handler))
        .route_layer(from_fn_with_state(state, auth_middleware::<MockStore>))
}

#[tokio::test]
async fn test_missing_auth() {
    let app = create_app(MockStore::default());
    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

    let body_bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: serde_json::Value = serde_json::from_slice(&body_bytes).unwrap();
    assert_eq!(body["error"]["code"], "unauthorized");
    assert_eq!(body["error"]["message"], "Authentication required");
}

#[tokio::test]
async fn test_malformed_auth() {
    let app = create_app(MockStore::default());
    let req = Request::builder()
        .uri("/")
        .header("Authorization", "Invalid token")
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_valid_auth() {
    let store = MockStore::default();
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();
    let session = Session::new(user_id, tenant_id, 3600, 3600).unwrap();
    let session_id_str = session.id().to_string();

    store.sessions.lock().await.push(session.clone());

    let app = create_app(store);
    let req = Request::builder()
        .uri("/")
        .header("Authorization", format!("Bearer {}", session_id_str))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_expired_auth() {
    // To test expiration, we can set up a mock store that returns a session that immediately fails validation
    let store = MockStore::default();
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();
    // 1 second TTL
    let session = Session::new(user_id, tenant_id, 1, 1).unwrap();

    // Instead of sleeping, let's just use the mock store
    let session_id_str = session.id().to_string();

    store.sessions.lock().await.push(session);

    // Actually we can't easily wait without blocking, and sleeping is slow.
    // However, wait... does it fail if we sleep for 2 seconds? Let's just do it.
    tokio::time::sleep(std::time::Duration::from_secs(2)).await;

    let app = create_app(store);
    let req = Request::builder()
        .uri("/")
        .header("Authorization", format!("Bearer {}", session_id_str))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_revoked_auth() {
    let store = MockStore::default();
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();
    let mut session = Session::new(user_id, tenant_id, 3600, 3600).unwrap();
    session.revoke("Compromised");
    let session_id_str = session.id().to_string();

    store.sessions.lock().await.push(session);

    let app = create_app(store);
    let req = Request::builder()
        .uri("/")
        .header("Authorization", format!("Bearer {}", session_id_str))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_cookie_auth() {
    let store = MockStore::default();
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();
    let session = Session::new(user_id, tenant_id, 3600, 3600).unwrap();
    let session_id_str = session.id().to_string();

    store.sessions.lock().await.push(session);

    let app = create_app(store);
    let req = Request::builder()
        .uri("/")
        .header("Cookie", format!("session_id={}", session_id_str))
        .body(Body::empty())
        .unwrap();
    let response = app.oneshot(req).await.unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

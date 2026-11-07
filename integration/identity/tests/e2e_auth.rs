#![forbid(unsafe_code)]

use axum::{
    body::Body,
    extract::Extension,
    http::{header, Request, StatusCode},
    response::IntoResponse,
    routing::get,
    Router,
};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

use identity::{
    rbac::{Role, RoleBinding, RoleType, Scope},
    session::Session,
    storage::SqliteIdentityStore,
    types::{TenantId, User, UserId},
};
use integration_identity::{
    auth_middleware, AuthenticatedUser, RequireRole, SqliteIdentityAdapter,
};

async fn protected_handler(user: Extension<AuthenticatedUser>) -> impl IntoResponse {
    (StatusCode::OK, format!("user_id: {}", user.user_id))
}

async fn admin_handler(_role: RequireRole) -> impl IntoResponse {
    (StatusCode::OK, "admin area")
}

fn setup_db() -> (SqliteIdentityAdapter, UserId, TenantId, Session) {
    let store = SqliteIdentityStore::in_memory().unwrap();
    let tenant_id = TenantId::new_v4();

    let user = User::new("testuser".into(), "test@example.com".into(), "hash".into()).unwrap();
    let user_id = user.id();
    store.create_user(&user).unwrap();

    let session = Session::new(user_id, tenant_id, 3600, 3600).unwrap();
    store.save_session(&session).unwrap();

    let binding = RoleBinding {
        user_id,
        role: Role::tenant_admin(),
        scope: Scope::Tenant { tenant_id },
    };
    store.save_role_binding(&binding).unwrap();

    (
        SqliteIdentityAdapter::new(store),
        user_id,
        tenant_id,
        session,
    )
}

fn create_app(store: SqliteIdentityAdapter, role: RoleType, scope: Scope) -> Router {
    let state = Arc::new(store);
    Router::new()
        .route("/protected", get(protected_handler))
        .route("/admin", get(admin_handler))
        .layer(Extension(role))
        .layer(Extension(scope))
        .layer(axum::middleware::from_fn_with_state(
            state,
            auth_middleware::<SqliteIdentityAdapter>,
        ))
}

#[tokio::test]
async fn test_e2e_login_success() {
    let (store, user_id, tenant_id, session) = setup_db();
    let app = create_app(store, RoleType::TenantAdmin, Scope::Tenant { tenant_id });

    let req = Request::builder()
        .uri("/protected")
        .header(header::AUTHORIZATION, format!("Bearer {}", session.id()))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);

    let body =
        String::from_utf8(res.into_body().collect().await.unwrap().to_bytes().to_vec()).unwrap();
    assert_eq!(body, format!("user_id: {}", user_id));
}

#[tokio::test]
async fn test_e2e_admin_access() {
    let (store, _, tenant_id, session) = setup_db();
    let app = create_app(store, RoleType::TenantAdmin, Scope::Tenant { tenant_id });

    let req = Request::builder()
        .uri("/admin")
        .header(header::AUTHORIZATION, format!("Bearer {}", session.id()))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_e2e_admin_denied() {
    let (store, _, _, session) = setup_db();
    let app = create_app(store, RoleType::SuperAdmin, Scope::Global);

    let req = Request::builder()
        .uri("/admin")
        .header(header::AUTHORIZATION, format!("Bearer {}", session.id()))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_e2e_revoked_session() {
    let store_raw = SqliteIdentityStore::in_memory().unwrap();
    let tenant_id = TenantId::new_v4();

    let user = User::new("t".into(), "t@example.com".into(), "h".into()).unwrap();
    store_raw.create_user(&user).unwrap();

    let mut session = Session::new(user.id(), tenant_id, 3600, 3600).unwrap();
    store_raw.save_session(&session).unwrap();

    store_raw
        .revoke_session(session.id(), "Compromised")
        .unwrap();
    session.revoke("Compromised"); // Update memory instance so it matches

    let adapter = SqliteIdentityAdapter::new(store_raw);
    let app = create_app(adapter, RoleType::TenantAdmin, Scope::Tenant { tenant_id });

    let req = Request::builder()
        .uri("/protected")
        .header(header::AUTHORIZATION, format!("Bearer {}", session.id()))
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_e2e_forged_principal() {
    let (store, _, tenant_id, _) = setup_db();
    let app = create_app(store, RoleType::TenantAdmin, Scope::Tenant { tenant_id });

    let req = Request::builder()
        .uri("/protected")
        .header(
            header::AUTHORIZATION,
            "Bearer 00000000-0000-0000-0000-000000000000",
        )
        .body(Body::empty())
        .unwrap();

    let res = app.oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

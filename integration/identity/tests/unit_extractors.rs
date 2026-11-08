#![forbid(unsafe_code)]

use axum::{
    body::Body,
    extract::Extension,
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use identity::{
    rbac::{Permission, Role, RoleBinding, RoleType, Scope},
    types::{TenantId, UserId},
};
use integration_identity::{AuthenticatedUser, RequirePermission, RequireRole};
use tower::ServiceExt;

#[tokio::test]
async fn test_authenticated_user_missing() {
    let app = Router::new().route("/", get(|_user: AuthenticatedUser| async { "Success" }));

    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_authenticated_user_present() {
    let user_id = UserId::new_v4();
    let auth_user = AuthenticatedUser {
        user_id,
        bindings: vec![],
    };

    let app = Router::new()
        .route(
            "/",
            get(move |user: AuthenticatedUser| async move {
                assert_eq!(user.user_id, user_id);
                "Success"
            }),
        )
        .layer(Extension(auth_user));

    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_require_permission_granted() {
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();

    let binding = RoleBinding {
        user_id,
        role: Role::tenant_admin(),
        scope: Scope::Tenant { tenant_id },
    };

    let auth_user = AuthenticatedUser {
        user_id,
        bindings: vec![binding],
    };

    let app = Router::new()
        .route("/", get(|_perm: RequirePermission| async { "Success" }))
        .layer(Extension(Permission::SiteCreate))
        .layer(Extension(Scope::Tenant { tenant_id }))
        .layer(Extension(auth_user));

    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_require_permission_denied() {
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();
    let other_tenant_id = TenantId::new_v4();

    let binding = RoleBinding {
        user_id,
        role: Role::tenant_admin(),
        scope: Scope::Tenant { tenant_id },
    };

    let auth_user = AuthenticatedUser {
        user_id,
        bindings: vec![binding],
    };

    let app = Router::new()
        .route("/", get(|_perm: RequirePermission| async { "Success" }))
        .layer(Extension(Permission::SiteCreate))
        .layer(Extension(Scope::Tenant {
            tenant_id: other_tenant_id,
        }))
        .layer(Extension(auth_user));

    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn test_require_role_granted() {
    let user_id = UserId::new_v4();

    let binding = RoleBinding {
        user_id,
        role: Role::super_admin(),
        scope: Scope::Global,
    };

    let auth_user = AuthenticatedUser {
        user_id,
        bindings: vec![binding],
    };

    let app = Router::new()
        .route("/", get(|_role: RequireRole| async { "Success" }))
        .layer(Extension(RoleType::SuperAdmin))
        .layer(Extension(Scope::Global))
        .layer(Extension(auth_user));

    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_require_role_denied() {
    let user_id = UserId::new_v4();

    let binding = RoleBinding {
        user_id,
        role: Role::viewer(),
        scope: Scope::Global,
    };

    let auth_user = AuthenticatedUser {
        user_id,
        bindings: vec![binding],
    };

    let app = Router::new()
        .route("/", get(|_role: RequireRole| async { "Success" }))
        .layer(Extension(RoleType::TenantAdmin))
        .layer(Extension(Scope::Global))
        .layer(Extension(auth_user));

    let req = Request::builder().uri("/").body(Body::empty()).unwrap();
    let res = app.oneshot(req).await.unwrap();

    assert_eq!(res.status(), StatusCode::FORBIDDEN);
}

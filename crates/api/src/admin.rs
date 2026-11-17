#![forbid(unsafe_code)]

use crate::node_jobs::{self, hash_token};
use crate::websites::{require_website_permission, website_authorization_context};
use crate::{AppState, ErrorDetail, ErrorResponse};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{delete, get};
use axum::{Json, Router};
use chrono::Utc;
use identity::{PasswordManager, Permission, Role, RoleBinding, Scope, User, UserId};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateUserRequest {
    username: String,
    email: String,
    password: String,
    role: String,
}

#[derive(Serialize)]
struct AdminUserView {
    id: String,
    username: String,
    email: String,
    status: String,
    roles: Vec<String>,
    created_at: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateTokenRequest {
    name: String,
    scopes: Vec<String>,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/admin/users", get(list_users).post(create_user))
        .route("/v1/admin/users/{id}", delete(delete_user))
        .route("/v1/admin/tokens", get(list_tokens).post(create_token))
        .route("/v1/admin/tokens/{id}", delete(revoke_token))
}

fn bad_request_msg(message: &'static str) -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "bad_request",
            message,
        },
    }
}

fn conflict_msg(message: &'static str) -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "conflict",
            message,
        },
    }
}

fn parse_role(role: &str) -> Result<Role, ErrorResponse> {
    match role.trim().to_ascii_lowercase().as_str() {
        "super_admin" | "superadmin" => Ok(Role::super_admin()),
        "tenant_admin" | "tenantadmin" | "admin" => Ok(Role::tenant_admin()),
        "site_operator" | "siteoperator" | "operator" => Ok(Role::site_operator()),
        "viewer" => Ok(Role::viewer()),
        _ => Err(bad_request_msg(
            "role must be one of: super_admin, tenant_admin, site_operator, viewer",
        )),
    }
}

async fn list_users(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let identity_store = state.identity_store.lock().await;
    let users = identity_store
        .list_users_for_tenant(context.tenant_id)
        .map_err(|_| node_jobs::internal_error())?;

    let mut views = Vec::with_capacity(users.len());
    for user in users {
        let bindings = identity_store
            .get_user_role_bindings(user.id())
            .map_err(|_| node_jobs::internal_error())?;
        let roles = bindings
            .into_iter()
            .map(|b| b.role.name)
            .collect::<Vec<_>>();
        views.push(AdminUserView {
            id: user.id().to_string(),
            username: user.username().to_string(),
            email: user.email().to_string(),
            status: format!("{:?}", user.status()).to_ascii_lowercase(),
            roles,
            created_at: user.created_at().to_rfc3339(),
        });
    }

    Ok(Json(json!({ "users": views })))
}

async fn create_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateUserRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let username = body.username.trim();
    let email = body.email.trim();
    if username.is_empty() || username.len() > 64 {
        return Err(bad_request_msg("username must be 1..=64 characters"));
    }
    if email.is_empty() || !email.contains('@') || email.len() > 128 {
        return Err(bad_request_msg("invalid email address"));
    }
    if body.password.len() < 8 || body.password.len() > 256 {
        return Err(bad_request_msg("password must be 8..=256 characters"));
    }
    let role = parse_role(&body.role)?;
    let password_hash = PasswordManager::hash_password(&body.password)
        .map_err(|_| bad_request_msg("password failed policy validation"))?;

    let user = User::new(username.to_string(), email.to_string(), password_hash)
        .map_err(|_| bad_request_msg("invalid user attributes"))?;

    let scope = Scope::Tenant {
        tenant_id: context.tenant_id,
    };
    let binding = RoleBinding {
        user_id: user.id(),
        role: role.clone(),
        scope,
    };

    {
        let identity_store = state.identity_store.lock().await;
        if identity_store
            .get_user_by_username(username)
            .map_err(|_| node_jobs::internal_error())?
            .is_some()
        {
            return Err(conflict_msg("username already exists"));
        }
        identity_store
            .create_user(&user)
            .map_err(|_| conflict_msg("username or email already exists"))?;
        identity_store
            .save_role_binding(&binding)
            .map_err(|_| node_jobs::internal_error())?;
    }

    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let now = Utc::now().to_rfc3339();
    let _ = state.control_store.lock().await.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "user.create",
        "user",
        &user.id().to_string(),
        "success",
        Some(&format!("{} ({})", user.username(), role.name)),
        &now,
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "user": AdminUserView {
                id: user.id().to_string(),
                username: user.username().to_string(),
                email: user.email().to_string(),
                status: "active".to_string(),
                roles: vec![role.name],
                created_at: user.created_at().to_rfc3339(),
            }
        })),
    ))
}

async fn delete_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id_str): Path<String>,
) -> Result<StatusCode, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let target_uuid = Uuid::parse_str(&id_str).map_err(|_| node_jobs::bad_request())?;
    let target_user_id = UserId::from(target_uuid);
    if context.user_id == Some(target_user_id) {
        return Err(bad_request_msg("cannot delete your own user account"));
    }

    let deleted = state
        .identity_store
        .lock()
        .await
        .delete_user_for_tenant(context.tenant_id, target_user_id)
        .map_err(|_| node_jobs::internal_error())?;
    if !deleted {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "not_found",
                message: "User not found",
            },
        });
    }

    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let now = Utc::now().to_rfc3339();
    let _ = state.control_store.lock().await.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "user.delete",
        "user",
        &target_uuid.to_string(),
        "success",
        None,
        &now,
    );

    Ok(StatusCode::NO_CONTENT)
}

async fn list_tokens(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let tokens = state
        .control_store
        .lock()
        .await
        .list_api_tokens(context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?;

    Ok(Json(json!({ "tokens": tokens })))
}

async fn create_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateTokenRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let name = body.name.trim();
    if name.is_empty() || name.len() > 64 {
        return Err(bad_request_msg("token name must be 1..=64 characters"));
    }
    if body.scopes.is_empty() || body.scopes.len() > 16 {
        return Err(bad_request_msg("provide 1..=16 scopes"));
    }
    let scopes: Vec<String> = body
        .scopes
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if scopes.is_empty() {
        return Err(bad_request_msg("scopes cannot be empty"));
    }

    let token_id = Uuid::new_v4();
    let raw_secret = format!(
        "fp_tok_{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    );
    let token_hash = hash_token(&raw_secret);
    let user_id = context
        .user_id
        .map(|u| u.to_uuid())
        .unwrap_or_else(|| context.tenant_id.to_uuid());
    let now = Utc::now().to_rfc3339();

    let mut store = state.control_store.lock().await;
    let record = store
        .create_api_token(
            token_id,
            context.tenant_id.to_uuid(),
            user_id,
            name,
            &token_hash,
            &scopes,
            &now,
        )
        .map_err(|_| node_jobs::internal_error())?;

    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let _ = store.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "token.create",
        "api_token",
        &token_id.to_string(),
        "success",
        Some(name),
        &now,
    );

    Ok((
        StatusCode::CREATED,
        Json(json!({
            "token": raw_secret,
            "record": record,
        })),
    ))
}

async fn revoke_token(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id_str): Path<String>,
) -> Result<StatusCode, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::TenantManageUsers, None)?;

    let token_id = Uuid::parse_str(&id_str).map_err(|_| node_jobs::bad_request())?;
    let now = Utc::now().to_rfc3339();
    let mut store = state.control_store.lock().await;
    let revoked = store
        .revoke_api_token(context.tenant_id.to_uuid(), token_id, &now)
        .map_err(|_| node_jobs::internal_error())?;
    if !revoked {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "not_found",
                message: "API token not found",
            },
        });
    }

    let actor = context
        .user_id
        .map(|u| u.to_string())
        .unwrap_or_else(|| "admin-token".to_string());
    let _ = store.record_audit_log(
        context.tenant_id.to_uuid(),
        &actor,
        "token.revoke",
        "api_token",
        &token_id.to_string(),
        "success",
        None,
        &now,
    );

    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{header::AUTHORIZATION, Request};
    use http_body_util::BodyExt;
    use tempfile::TempDir;
    use tower::ServiceExt;

    const ADMIN_TOKEN: &str = "admin-test-token";
    const ADMIN_TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";

    fn test_app() -> (Router, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("admin.sqlite");
        let identity_store = std::sync::Arc::new(tokio::sync::Mutex::new(
            identity::SqliteIdentityStore::open(&db_path).unwrap(),
        ));
        let admin_tenant_id = Uuid::from_u128(1);
        let mut control_store = fusionpanel_control_store::ControlStore::new(&db_path).unwrap();
        control_store
            .create_resource(
                admin_tenant_id,
                admin_tenant_id,
                fusionpanel_control_store::OwnershipMode::PlatformManaged,
                json!({ "name": "Admin Tenant" }),
            )
            .unwrap();
        let state = AppState {
            admin_token: std::sync::Arc::new(ADMIN_TOKEN.to_string()),
            identity_store,
            control_store: std::sync::Arc::new(tokio::sync::Mutex::new(control_store)),
            jobs_store: std::sync::Arc::new(tokio::sync::Mutex::new(
                fusionpanel_jobs::JobStore::new(&db_path).unwrap(),
            )),
            frontend_dir: None,
        };
        (crate::app(state), dir)
    }

    async fn json_body(res: axum::response::Response) -> Value {
        let bytes = res.into_body().collect().await.unwrap().to_bytes();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[tokio::test]
    async fn rbac_users_and_scoped_api_tokens_enforce_permissions() {
        let (app, _dir) = test_app();

        // 1. Create a Viewer user
        let res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/admin/users")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "username": "readonly-op",
                            "email": "readonly@example.com",
                            "password": "StrongPassword123!",
                            "role": "viewer"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::CREATED);

        // 2. Login as Viewer and confirm 403 on /v1/admin/users
        let login_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/auth/login")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "username": "readonly-op",
                            "password": "StrongPassword123!"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(login_res.status(), StatusCode::OK);
        let cookie = login_res
            .headers()
            .get_all("set-cookie")
            .iter()
            .filter_map(|v| v.to_str().ok())
            .map(|s| s.split(';').next().unwrap_or(s).to_string())
            .collect::<Vec<_>>()
            .join("; ");

        let viewer_admin = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/admin/users")
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(viewer_admin.status(), StatusCode::FORBIDDEN);

        // 3. Create a read-only scoped API token
        let tok_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/admin/tokens")
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "name": "sites-reader",
                            "scopes": ["sites:read"]
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(tok_res.status(), StatusCode::CREATED);
        let tok_json = json_body(tok_res).await;
        let raw_tok = tok_json["token"].as_str().unwrap().to_string();
        let tok_id = tok_json["record"]["id"].as_str().unwrap().to_string();

        // Allowed: GET /v1/websites
        let read_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/websites")
                    .header(AUTHORIZATION, format!("Bearer {raw_tok}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(read_res.status(), StatusCode::OK);

        // Forbidden: POST /v1/websites (token only has sites:read)
        let write_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/websites")
                    .header(AUTHORIZATION, format!("Bearer {raw_tok}"))
                    .header("Idempotency-Key", "tok-write-1")
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "domain": "blocked.local",
                            "kind": "static",
                            "document_root": "/srv/fusionpanel/sites/blocked.local"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(write_res.status(), StatusCode::FORBIDDEN);

        // Revoke token -> 401
        let rev_res = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri(format!("/v1/admin/tokens/{tok_id}"))
                    .header(AUTHORIZATION, format!("Bearer {ADMIN_TOKEN}"))
                    .header("x-fusionpanel-tenant-id", ADMIN_TENANT_ID)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(rev_res.status(), StatusCode::NO_CONTENT);

        let after_rev = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/v1/websites")
                    .header(AUTHORIZATION, format!("Bearer {raw_tok}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after_rev.status(), StatusCode::UNAUTHORIZED);
    }
}

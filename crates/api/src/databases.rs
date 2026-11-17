#![forbid(unsafe_code)]

use super::{
    node_jobs,
    websites::{require_website_permission, website_authorization_context},
    AppState, ErrorDetail, ErrorResponse,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use fusionpanel_agent_protocol::{
    valid_database_identifier, DatabaseApplyPayload, DatabaseDeletePayload, DatabaseEngineKind,
    DatabaseUserCreatePayload, Operation, Request,
};
use fusionpanel_control_store::{DatabaseEngine, DatabaseOperation, StoreError};
use fusionpanel_jobs::{Job, JobState};
use identity::Permission;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateDatabaseRequest {
    name: String,
    engine: DatabaseEngineKind,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct CreateDatabaseUserRequest {
    #[serde(default)]
    username: Option<String>,
    #[serde(default)]
    password: Option<String>,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/databases", get(list_databases).post(create_database))
        .route(
            "/api/v1/databases",
            get(list_databases).post(create_database),
        )
        .route(
            "/v1/databases/{id}",
            get(get_database).delete(delete_database),
        )
        .route(
            "/api/v1/databases/{id}",
            get(get_database).delete(delete_database),
        )
        .route("/v1/databases/{id}/users", post(create_database_user))
        .route("/api/v1/databases/{id}/users", post(create_database_user))
}

async fn list_databases(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DatabaseManage, None)?;
    let databases = state
        .control_store
        .lock()
        .await
        .list_databases_for_tenant(context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "databases": databases })))
}

async fn get_database(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DatabaseManage, None)?;
    let database_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    let store = state.control_store.lock().await;
    let database = store
        .get_database_for_tenant(database_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    let users = store
        .list_database_users_for_tenant(database_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "database": database, "users": users })))
}

async fn create_database(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateDatabaseRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DatabaseManage, None)?;
    let key = idempotency_key(&headers)?;

    let name = body.name.trim().to_string();
    if !valid_database_identifier(&name) {
        return Err(bad_request(
            "Database name must be 1-63 alphanumeric/underscore characters and not reserved",
        ));
    }
    let engine = match body.engine {
        DatabaseEngineKind::Postgresql => DatabaseEngine::Postgresql,
        DatabaseEngineKind::Mariadb => DatabaseEngine::Mariadb,
    };

    let fingerprint = format!("{}:{}:{}", context.tenant_id, engine.as_str(), name);
    let now = Utc::now().to_rfc3339();
    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_database_create(
            context.tenant_id.to_uuid(),
            &name,
            engine,
            key,
            &fingerprint,
            &now,
        )
        .map_err(map_store_error)?;

    let request = Request {
        version: 1,
        request_id: reservation.request_id,
        operation: Operation::DatabaseApply(DatabaseApplyPayload {
            database_id: reservation.database.id,
            name: reservation.database.name.clone(),
            engine: body.engine,
        }),
    };
    let job = enqueue_database_job(&state, key, &request, reservation.existing_job_id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "database": reservation.database,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn delete_database(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DatabaseManage, None)?;
    let key = idempotency_key(&headers)?;
    let database_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    let fingerprint = format!("delete:{tenant_id}:{database_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_database_delete(database_id, tenant_id, key, &fingerprint, &now)
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let engine = match reservation.database.engine {
        DatabaseEngine::Postgresql => DatabaseEngineKind::Postgresql,
        DatabaseEngine::Mariadb => DatabaseEngineKind::Mariadb,
    };
    let request = Request {
        version: 1,
        request_id: reservation.request_id,
        operation: Operation::DatabaseDelete(DatabaseDeletePayload {
            database_id: reservation.database.id,
            name: reservation.database.name.clone(),
            engine,
        }),
    };
    let job = enqueue_database_job(&state, key, &request, reservation.existing_job_id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "database": reservation.database,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn create_database_user(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<CreateDatabaseUserRequest>>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::DatabaseManage, None)?;
    let key = idempotency_key(&headers)?;
    let database_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    let req = body.map(|b| b.0).unwrap_or_default();

    let username = match req.username {
        Some(raw) => raw.trim().to_string(),
        None => {
            let suffix = &Uuid::new_v4().simple().to_string()[..8];
            format!("fp_{suffix}")
        }
    };
    if !valid_database_identifier(&username) {
        return Err(bad_request(
            "Database username must be 1-63 alphanumeric/underscore characters and not reserved",
        ));
    }
    let password = match req.password {
        Some(raw) => raw,
        None => {
            let raw = Uuid::new_v4().simple().to_string();
            format!("Fp!{raw}")
        }
    };

    let fingerprint = format!("user:{tenant_id}:{database_id}:{username}");
    let now = Utc::now().to_rfc3339();
    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_database_user_create(database_id, tenant_id, &username, key, &fingerprint, &now)
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let user = reservation.user.ok_or_else(node_jobs::internal_error)?;
    let engine = match reservation.database.engine {
        DatabaseEngine::Postgresql => DatabaseEngineKind::Postgresql,
        DatabaseEngine::Mariadb => DatabaseEngineKind::Mariadb,
    };

    let request = Request {
        version: 1,
        request_id: reservation.request_id,
        operation: Operation::DatabaseUserCreate(DatabaseUserCreatePayload {
            database_id: reservation.database.id,
            database_name: reservation.database.name.clone(),
            engine,
            username: user.username.clone(),
            password: password.clone(),
        }),
    };
    request
        .validate()
        .map_err(|_| bad_request("Invalid database user payload"))?;

    let job = enqueue_database_job(&state, key, &request, reservation.existing_job_id).await?;
    let (host, port) = default_connection_endpoint(reservation.database.engine);

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "user": user,
            "credentials": {
                "host": host,
                "port": port,
                "database": reservation.database.name,
                "username": user.username,
                "password": if reservation.already_existed { "[REDACTED]" } else { &password },
            },
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

fn default_connection_endpoint(engine: DatabaseEngine) -> (String, u16) {
    match engine {
        DatabaseEngine::Postgresql => (
            std::env::var("FUSIONPANEL_PG_PUBLIC_HOST").unwrap_or_else(|_| "127.0.0.1".to_string()),
            std::env::var("FUSIONPANEL_PG_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(5432),
        ),
        DatabaseEngine::Mariadb => (
            std::env::var("FUSIONPANEL_MARIADB_PUBLIC_HOST")
                .unwrap_or_else(|_| "127.0.0.1".to_string()),
            std::env::var("FUSIONPANEL_MARIADB_PORT")
                .ok()
                .and_then(|v| v.parse::<u16>().ok())
                .unwrap_or(3306),
        ),
    }
}

async fn enqueue_database_job(
    state: &AppState,
    idempotency_key: &str,
    request: &Request,
    existing_job_id: Option<Uuid>,
) -> Result<Job, ErrorResponse> {
    if let Some(job_id) = existing_job_id {
        return state
            .jobs_store
            .lock()
            .await
            .get(&job_id.to_string())
            .map_err(node_jobs::map_job_error);
    }
    request
        .validate()
        .map_err(|_| bad_request("Invalid database operation"))?;
    let payload = serde_json::to_value(request).map_err(|_| node_jobs::internal_error())?;
    let job_key = format!("database:{idempotency_key}");
    let (job_id, _) = state
        .jobs_store
        .lock()
        .await
        .create_idempotent("agent_operation", &payload, &job_key, &payload)
        .map_err(node_jobs::map_job_error)?;
    let job_uuid = Uuid::parse_str(&job_id).map_err(|_| node_jobs::internal_error())?;
    state
        .control_store
        .lock()
        .await
        .attach_database_operation_job(idempotency_key, job_uuid)
        .map_err(|_| node_jobs::internal_error())?;
    state
        .jobs_store
        .lock()
        .await
        .get(&job_id)
        .map_err(node_jobs::map_job_error)
}

pub(super) async fn record_completion(state: &AppState, job: &Job) -> Result<(), ErrorResponse> {
    if job.kind != "agent_operation" {
        return Ok(());
    }
    let Ok(request) = serde_json::from_value::<Request>(job.payload.clone()) else {
        return Ok(());
    };
    let succeeded = job.state == JobState::Succeeded;
    let error_text = if succeeded {
        None
    } else {
        Some(
            job.error_text
                .as_deref()
                .unwrap_or("Database operation failed"),
        )
    };
    let now = Utc::now().to_rfc3339();
    let mut store = state.control_store.lock().await;
    match request.operation {
        Operation::DatabaseApply(payload) => {
            store
                .complete_database_operation(
                    DatabaseOperation::Apply,
                    payload.database_id,
                    request.request_id,
                    succeeded,
                    error_text,
                    &now,
                )
                .map_err(|_| node_jobs::internal_error())?;
        }
        Operation::DatabaseDelete(payload) => {
            store
                .complete_database_operation(
                    DatabaseOperation::Delete,
                    payload.database_id,
                    request.request_id,
                    succeeded,
                    error_text,
                    &now,
                )
                .map_err(|_| node_jobs::internal_error())?;
        }
        Operation::DatabaseUserCreate(payload) => {
            store
                .complete_database_operation(
                    DatabaseOperation::UserCreate,
                    payload.database_id,
                    request.request_id,
                    succeeded,
                    error_text,
                    &now,
                )
                .map_err(|_| node_jobs::internal_error())?;
        }
        _ => {}
    }
    Ok(())
}

fn idempotency_key(headers: &HeaderMap) -> Result<&str, ErrorResponse> {
    headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty() && value.len() <= 255)
        .ok_or_else(|| bad_request("Idempotency-Key header is required"))
}

fn parse_uuid(value: &str) -> Result<Uuid, ErrorResponse> {
    Uuid::parse_str(value).map_err(|_| bad_request("Invalid UUID"))
}

fn bad_request(message: &'static str) -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "bad_request",
            message,
        },
    }
}

fn not_found() -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "not_found",
            message: "Database not found",
        },
    }
}

fn map_store_error(error: StoreError) -> ErrorResponse {
    match error {
        StoreError::IdempotencyConflict => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Idempotency-Key was reused with a different database payload",
            },
        },
        StoreError::DatabaseConflict(_) | StoreError::DatabaseOperationInProgress => {
            ErrorResponse {
                error: ErrorDetail {
                    code: "conflict",
                    message: "Database request conflicts with existing state",
                },
            }
        }
        _ => node_jobs::internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app;
    use axum::{body::Body, http::Request as HttpRequest};
    use fusionpanel_control_store::ControlStore;
    use fusionpanel_jobs::JobStore;
    use http_body_util::BodyExt;
    use identity::{Role, RoleBinding, Scope, Session, SqliteIdentityStore, TenantId, User};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            admin_token: Arc::new("test-admin-token".to_string()),
            identity_store: Arc::new(Mutex::new(SqliteIdentityStore::in_memory().unwrap())),
            control_store: Arc::new(Mutex::new(ControlStore::new_in_memory().unwrap())),
            jobs_store: Arc::new(Mutex::new(JobStore::in_memory().unwrap())),
            frontend_dir: None,
        }
    }

    async fn create_session(state: &AppState, tenant_id: Uuid, role: Role) -> String {
        let user = User::new(
            format!("db-test-{}", Uuid::new_v4()),
            format!("{}@example.test", Uuid::new_v4()),
            "synthetic-password-hash".to_string(),
        )
        .unwrap();
        let tid = TenantId::from(tenant_id);
        let session = Session::new(user.id(), tid, 3600, 1800).unwrap();
        let token = session.id().to_string();
        let store = state.identity_store.lock().await;
        store.create_user(&user).unwrap();
        store
            .save_role_binding(&RoleBinding {
                user_id: user.id(),
                role,
                scope: Scope::Tenant { tenant_id: tid },
            })
            .unwrap();
        store.save_session(&session).unwrap();
        token
    }

    #[tokio::test]
    async fn database_routes_validate_auth_and_redact_passwords() {
        let state = test_state();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let session_a = create_session(&state, tenant_a, Role::tenant_admin()).await;
        let session_b = create_session(&state, tenant_b, Role::tenant_admin()).await;
        let viewer_a = create_session(&state, tenant_a, Role::viewer()).await;
        let router = app(state.clone());

        // Viewer lacks DatabaseManage
        let denied = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri("/v1/databases")
                    .header("authorization", format!("Bearer {viewer_a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);

        // Tenant A creates a PostgreSQL database
        let create_resp = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/databases")
                    .header("authorization", format!("Bearer {session_a}"))
                    .header("content-type", "application/json")
                    .header("idempotency-key", "db-create-1")
                    .body(Body::from(r#"{"name":"app_pg","engine":"postgresql"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::ACCEPTED);
        let create_json: serde_json::Value =
            serde_json::from_slice(&create_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        let db_id = create_json["database"]["id"].as_str().unwrap().to_string();
        let job_id = create_json["job"]["id"].as_str().unwrap().to_string();

        // Tenant B cannot see Tenant A's database
        let cross_tenant = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/databases/{db_id}"))
                    .header("authorization", format!("Bearer {session_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cross_tenant.status(), StatusCode::NOT_FOUND);

        // Complete database creation job
        {
            let mut jobs = state.jobs_store.lock().await;
            jobs.claim("node-1").unwrap();
            jobs.transition_for_worker(&job_id, "node-1", JobState::Succeeded, None)
                .unwrap();
            let completed = jobs.get(&job_id).unwrap();
            drop(jobs);
            assert!(record_completion(&state, &completed).await.is_ok());
        }

        // Create database user and verify password is returned once & redacted in job
        let user_resp = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/databases/{db_id}/users"))
                    .header("authorization", format!("Bearer {session_a}"))
                    .header("content-type", "application/json")
                    .header("idempotency-key", "db-user-1")
                    .body(Body::from(r#"{"username":"app_user"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(user_resp.status(), StatusCode::ACCEPTED);
        let user_json: serde_json::Value =
            serde_json::from_slice(&user_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_ne!(user_json["credentials"]["password"], "[REDACTED]");
        assert_eq!(
            user_json["job"]["payload"]["payload"]["password"],
            "[REDACTED]"
        );
    }
}

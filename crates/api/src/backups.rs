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
    BackupCreatePayload, BackupDestination, BackupRestorePayload, BackupTarget, DatabaseEngineKind,
    Operation, Request,
};
use fusionpanel_control_store::{BackupOperation, DatabaseEngine, StoreError, WebsiteSpec};
use fusionpanel_jobs::{Job, JobState};
use identity::Permission;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum CreateBackupTargetRequest {
    Site { site_id: Uuid },
    Database { database_id: Uuid },
    Bundle { site_id: Uuid, database_id: Uuid },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateBackupRequest {
    target: CreateBackupTargetRequest,
    destination: BackupDestination,
}

#[derive(Deserialize, Default)]
#[serde(deny_unknown_fields)]
struct RestoreBackupRequest {
    #[serde(default)]
    destination: Option<BackupDestination>,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/backups", get(list_backups).post(create_backup))
        .route("/api/v1/backups", get(list_backups).post(create_backup))
        .route("/v1/backups/{id}", get(get_backup))
        .route("/api/v1/backups/{id}", get(get_backup))
        .route("/v1/backups/{id}/restore", post(restore_backup))
        .route("/api/v1/backups/{id}/restore", post(restore_backup))
}

async fn list_backups(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::BackupCreate, None)?;
    let backups = state
        .control_store
        .lock()
        .await
        .list_backups_for_tenant(context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "backups": backups })))
}

async fn get_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::BackupCreate, None)?;
    let backup_id = parse_uuid(&id)?;
    let backup = state
        .control_store
        .lock()
        .await
        .get_backup_for_tenant(backup_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "backup": backup })))
}

async fn create_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateBackupRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::BackupCreate, None)?;
    let key = idempotency_key(&headers)?;
    let tenant_id = context.tenant_id.to_uuid();

    body.destination
        .validate()
        .map_err(|_| bad_request("Invalid backup destination"))?;

    let target = resolve_backup_target(&state, tenant_id, body.target).await?;
    let backup_id = Uuid::new_v4();
    let archive_key = format!("tenant-{tenant_id}/backup-{backup_id}.tar.gz");
    let (dest_kind, dest_summary) = summarize_destination(&body.destination);
    let target_json = serde_json::to_value(&target).map_err(|_| node_jobs::internal_error())?;
    let fingerprint = format!("create:{tenant_id}:{target_json}:{dest_kind}:{dest_summary}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_backup_create(
            tenant_id,
            backup_id,
            &archive_key,
            target_json,
            dest_kind,
            &dest_summary,
            key,
            &fingerprint,
            &now,
        )
        .map_err(map_store_error)?;

    let resolved_target: BackupTarget = serde_json::from_value(reservation.backup.target.clone())
        .map_err(|_| node_jobs::internal_error())?;
    let request = Request {
        version: 1,
        request_id: reservation.request_id,
        operation: Operation::BackupCreate(BackupCreatePayload {
            backup_id: reservation.backup.id,
            tenant_id,
            archive_key: reservation.backup.archive_key.clone(),
            target: resolved_target,
            destination: body.destination,
        }),
    };
    let job = enqueue_backup_job(&state, key, &request, reservation.existing_job_id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "backup": reservation.backup,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn restore_backup(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Option<Json<RestoreBackupRequest>>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::BackupRestore, None)?;
    let key = idempotency_key(&headers)?;
    let backup_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    let req = body.map(|b| b.0).unwrap_or_default();

    let existing = state
        .control_store
        .lock()
        .await
        .get_backup_for_tenant(backup_id, tenant_id)
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;

    let destination = match req.destination {
        Some(dest) => dest,
        None if existing.destination_kind == "local" => BackupDestination::Local {
            base_dir: existing.destination_summary.clone(),
        },
        None => {
            return Err(bad_request(
                "Destination with S3 credentials is required to restore an S3 backup",
            ))
        }
    };
    destination
        .validate()
        .map_err(|_| bad_request("Invalid backup destination"))?;

    let (dest_kind, dest_summary) = summarize_destination(&destination);
    let fingerprint = format!("restore:{tenant_id}:{backup_id}:{dest_kind}:{dest_summary}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_backup_restore(backup_id, tenant_id, key, &fingerprint, &now)
        .map_err(map_store_error)?
        .ok_or_else(not_found)?;

    let resolved_target: BackupTarget = serde_json::from_value(reservation.backup.target.clone())
        .map_err(|_| node_jobs::internal_error())?;
    let request = Request {
        version: 1,
        request_id: reservation.request_id,
        operation: Operation::BackupRestore(BackupRestorePayload {
            backup_id: reservation.backup.id,
            tenant_id,
            archive_key: reservation.backup.archive_key.clone(),
            target: resolved_target,
            destination,
            expected_sha256: reservation.backup.manifest_sha256.clone(),
        }),
    };
    let job = enqueue_backup_job(&state, key, &request, reservation.existing_job_id).await?;
    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "backup": reservation.backup,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn resolve_backup_target(
    state: &AppState,
    tenant_id: Uuid,
    target: CreateBackupTargetRequest,
) -> Result<BackupTarget, ErrorResponse> {
    let store = state.control_store.lock().await;
    match target {
        CreateBackupTargetRequest::Site { site_id } => {
            let site =
                store
                    .get_website_for_tenant(site_id, tenant_id)
                    .map_err(|err| match err {
                        StoreError::WebsiteNotFound(_) => bad_request("Referenced site not found"),
                        _ => node_jobs::internal_error(),
                    })?;
            let WebsiteSpec::Static { document_root } = site.spec else {
                return Err(bad_request(
                    "Only static websites with a document_root can be backed up",
                ));
            };
            Ok(BackupTarget::Site {
                site_id: site.id,
                domain: site.domain,
                document_root,
            })
        }
        CreateBackupTargetRequest::Database { database_id } => {
            let db = store
                .get_database_for_tenant(database_id, tenant_id)
                .map_err(|_| node_jobs::internal_error())?
                .ok_or_else(|| bad_request("Referenced database not found"))?;
            Ok(BackupTarget::Database {
                database_id: db.id,
                name: db.name,
                engine: map_db_engine(db.engine),
            })
        }
        CreateBackupTargetRequest::Bundle {
            site_id,
            database_id,
        } => {
            let site =
                store
                    .get_website_for_tenant(site_id, tenant_id)
                    .map_err(|err| match err {
                        StoreError::WebsiteNotFound(_) => bad_request("Referenced site not found"),
                        _ => node_jobs::internal_error(),
                    })?;
            let WebsiteSpec::Static { document_root } = site.spec else {
                return Err(bad_request(
                    "Only static websites with a document_root can be backed up",
                ));
            };
            let db = store
                .get_database_for_tenant(database_id, tenant_id)
                .map_err(|_| node_jobs::internal_error())?
                .ok_or_else(|| bad_request("Referenced database not found"))?;
            Ok(BackupTarget::Bundle {
                site_id: site.id,
                domain: site.domain,
                document_root,
                database_id: db.id,
                database_name: db.name,
                engine: map_db_engine(db.engine),
            })
        }
    }
}

fn map_db_engine(engine: DatabaseEngine) -> DatabaseEngineKind {
    match engine {
        DatabaseEngine::Postgresql => DatabaseEngineKind::Postgresql,
        DatabaseEngine::Mariadb => DatabaseEngineKind::Mariadb,
    }
}

fn summarize_destination(dest: &BackupDestination) -> (&'static str, String) {
    match dest {
        BackupDestination::Local { base_dir } => ("local", base_dir.clone()),
        BackupDestination::S3 {
            endpoint,
            bucket,
            prefix,
            ..
        } => {
            let summary = match prefix.as_deref() {
                Some(p) if !p.is_empty() => format!("{endpoint}/{bucket}/{p}"),
                _ => format!("{endpoint}/{bucket}"),
            };
            ("s3", summary)
        }
    }
}

async fn enqueue_backup_job(
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
        .map_err(|_| bad_request("Invalid backup request"))?;
    let payload = serde_json::to_value(request).map_err(|_| node_jobs::internal_error())?;
    let job_key = format!("backup:{idempotency_key}");
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
        .attach_backup_operation_job(idempotency_key, job_uuid)
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
                .unwrap_or("Backup operation failed"),
        )
    };
    let manifest_sha256 = if succeeded {
        let logs = state
            .jobs_store
            .lock()
            .await
            .list_logs(&job.id)
            .unwrap_or_default();
        logs.iter().rev().find_map(|entry| {
            entry
                .text
                .split_whitespace()
                .find_map(|tok| tok.strip_prefix("sha256="))
                .filter(|sha| sha.len() == 64 && sha.chars().all(|c| c.is_ascii_hexdigit()))
                .map(str::to_string)
        })
    } else {
        None
    };

    let now = Utc::now().to_rfc3339();
    let mut store = state.control_store.lock().await;
    match request.operation {
        Operation::BackupCreate(payload) => {
            store
                .complete_backup_operation(
                    BackupOperation::Create,
                    payload.backup_id,
                    request.request_id,
                    succeeded,
                    manifest_sha256.as_deref(),
                    error_text,
                    &now,
                )
                .map_err(|_| node_jobs::internal_error())?;
        }
        Operation::BackupRestore(payload) => {
            store
                .complete_backup_operation(
                    BackupOperation::Restore,
                    payload.backup_id,
                    request.request_id,
                    succeeded,
                    None,
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
            message: "Backup not found",
        },
    }
}

fn map_store_error(error: StoreError) -> ErrorResponse {
    match error {
        StoreError::IdempotencyConflict => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Idempotency-Key was reused with a different backup payload",
            },
        },
        StoreError::DatabaseOperationInProgress => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Backup request conflicts with existing state",
            },
        },
        _ => node_jobs::internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app;
    use axum::{body::Body, http::Request as HttpRequest};
    use fusionpanel_control_store::{ControlStore, DatabaseEngine};
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
            format!("bk-test-{}", Uuid::new_v4()),
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
    async fn backup_routes_enforce_tenant_isolation_and_redact_s3_secrets() {
        let state = test_state();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let session_a = create_session(&state, tenant_a, Role::tenant_admin()).await;
        let session_b = create_session(&state, tenant_b, Role::tenant_admin()).await;

        let db_res = state
            .control_store
            .lock()
            .await
            .reserve_database_create(
                tenant_a,
                "bk_app_db",
                DatabaseEngine::Postgresql,
                "db-k1",
                "fp-k1",
                "2026-11-17T12:00:00Z",
            )
            .unwrap();

        let router = app(state.clone());
        let create_resp = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/backups")
                    .header("authorization", format!("Bearer {session_a}"))
                    .header("content-type", "application/json")
                    .header("idempotency-key", "bk-create-1")
                    .body(Body::from(
                        json!({
                            "target": { "kind": "database", "database_id": db_res.database.id },
                            "destination": {
                                "kind": "s3",
                                "endpoint": "http://127.0.0.1:9000",
                                "bucket": "backups",
                                "region": "us-east-1",
                                "access_key": "minioadmin",
                                "secret_key": "super-secret-s3-key",
                                "prefix": "sprint6"
                            }
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_resp.status(), StatusCode::ACCEPTED);
        let create_json: serde_json::Value =
            serde_json::from_slice(&create_resp.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(
            create_json["job"]["payload"]["payload"]["destination"]["secret_key"],
            "[REDACTED]"
        );

        let backup_id = create_json["backup"]["id"].as_str().unwrap().to_string();
        let cross_tenant = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/backups/{backup_id}"))
                    .header("authorization", format!("Bearer {session_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(cross_tenant.status(), StatusCode::NOT_FOUND);
    }
}

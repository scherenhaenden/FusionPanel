#![forbid(unsafe_code)]

use super::{
    node_jobs,
    websites::{require_website_permission, website_authorization_context},
    AppState, ErrorDetail, ErrorResponse,
};
use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    routing::{delete, get, post},
    Json, Router,
};
use chrono::Utc;
use fusionpanel_agent_protocol::{
    CronApplyPayload, CronDeletePayload, CronJobSpec, CronRunNowPayload, Operation, Request,
};
use fusionpanel_control_store::{CronJobRecord, CronOperation, StoreError};
use fusionpanel_jobs::{Job, JobState};
use identity::Permission;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateCronRequest {
    name: String,
    schedule: String,
    command: String,
    #[serde(default = "default_enabled")]
    enabled: bool,
}

fn default_enabled() -> bool {
    true
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/cron", get(list_cron_jobs).post(create_cron_job))
        .route("/api/v1/cron", get(list_cron_jobs).post(create_cron_job))
        .route("/v1/cron/{id}", delete(delete_cron_job))
        .route("/api/v1/cron/{id}", delete(delete_cron_job))
        .route("/v1/cron/{id}/run", post(run_cron_job_now))
        .route("/api/v1/cron/{id}/run", post(run_cron_job_now))
}

async fn list_cron_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteRead, None)?;
    let cron_jobs = state
        .control_store
        .lock()
        .await
        .list_cron_jobs(context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "cron_jobs": cron_jobs })))
}

async fn create_cron_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateCronRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteUpdate, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let tenant_id = context.tenant_id.to_uuid();

    let name = body.name.trim().to_string();
    let schedule = body.schedule.trim().to_string();
    let command = body.command.trim().to_string();
    let probe = CronJobSpec {
        cron_id: Uuid::new_v4(),
        name: name.clone(),
        schedule: schedule.clone(),
        command: command.clone(),
        enabled: body.enabled,
    };
    probe
        .validate()
        .map_err(|_| bad_request("Invalid cron name, schedule, or command"))?;

    let scoped_key = format!("cron-create:{tenant_id}:{idempotency_key}");
    let fingerprint = format!("cron.create:{name}:{schedule}:{command}:{}", body.enabled);
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_cron_create(
            &scoped_key,
            &fingerprint,
            tenant_id,
            &name,
            &schedule,
            &command,
            body.enabled,
            &now,
        )
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::CronApply(CronApplyPayload {
                tenant_id,
                cron_id: reservation.cron_job.id,
                jobs: reservation.tenant_jobs.iter().map(to_spec).collect(),
            }),
        };
        enqueue_cron_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "cron_job": reservation.cron_job, "job": job })),
    ))
}

async fn delete_cron_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteUpdate, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let cron_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    ensure_cron_ownership(&state, cron_id, tenant_id).await?;

    let scoped_key = format!("cron-delete:{cron_id}:{idempotency_key}");
    let fingerprint = format!("cron.delete:{cron_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_cron_delete(&scoped_key, &fingerprint, cron_id, &now)
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::CronDelete(CronDeletePayload {
                tenant_id,
                cron_id,
                jobs: reservation.tenant_jobs.iter().map(to_spec).collect(),
            }),
        };
        enqueue_cron_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "cron_job": reservation.cron_job, "job": job })),
    ))
}

async fn run_cron_job_now(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteUpdate, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let cron_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    let existing = ensure_cron_ownership(&state, cron_id, tenant_id).await?;

    let scoped_key = format!("cron-run:{cron_id}:{idempotency_key}");
    let fingerprint = format!("cron.run_now:{cron_id}:{}", existing.command);

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_cron_run_now(&scoped_key, &fingerprint, cron_id)
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::CronRunNow(CronRunNowPayload {
                tenant_id,
                cron_id,
                command: existing.command,
            }),
        };
        enqueue_cron_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({ "cron_job": reservation.cron_job, "job": job })),
    ))
}

fn to_spec(rec: &CronJobRecord) -> CronJobSpec {
    CronJobSpec {
        cron_id: rec.id,
        name: rec.name.clone(),
        schedule: rec.schedule.clone(),
        command: rec.command.clone(),
        enabled: rec.enabled,
    }
}

async fn enqueue_cron_job(
    state: &AppState,
    scoped_key: &str,
    req: Request,
) -> Result<Job, ErrorResponse> {
    req.validate()
        .map_err(|_| bad_request("Invalid cron payload"))?;
    let payload_value = serde_json::to_value(&req).map_err(|_| node_jobs::internal_error())?;
    let job_key = format!("cron-job:{scoped_key}");
    let (job_id, _) = state
        .jobs_store
        .lock()
        .await
        .create_idempotent("agent_operation", &payload_value, &job_key, &payload_value)
        .map_err(node_jobs::map_job_error)?;
    let job_uuid = Uuid::parse_str(&job_id).map_err(|_| node_jobs::internal_error())?;
    state
        .control_store
        .lock()
        .await
        .set_cron_operation_job_id(scoped_key, job_uuid)
        .map_err(|_| node_jobs::internal_error())?;
    state
        .jobs_store
        .lock()
        .await
        .get(&job_id)
        .map_err(node_jobs::map_job_error)
}

async fn load_existing_job(state: &AppState, job_id: Option<Uuid>) -> Result<Job, ErrorResponse> {
    let job_id = job_id.ok_or_else(node_jobs::internal_error)?;
    state
        .jobs_store
        .lock()
        .await
        .get(&job_id.to_string())
        .map_err(node_jobs::map_job_error)
}

async fn ensure_cron_ownership(
    state: &AppState,
    cron_id: Uuid,
    tenant_id: Uuid,
) -> Result<CronJobRecord, ErrorResponse> {
    state
        .control_store
        .lock()
        .await
        .get_cron_job(cron_id)
        .map_err(|_| node_jobs::internal_error())?
        .filter(|c| c.tenant_id == tenant_id)
        .ok_or_else(not_found)
}

pub(super) async fn record_completion(state: &AppState, job: &Job) -> Result<(), ErrorResponse> {
    let Ok(request) = serde_json::from_value::<Request>(job.payload.clone()) else {
        return Ok(());
    };
    let op = match &request.operation {
        Operation::CronApply(_) => CronOperation::Apply,
        Operation::CronDelete(_) => CronOperation::Delete,
        Operation::CronRunNow(_) => CronOperation::RunNow,
        _ => return Ok(()),
    };
    let succeeded = job.state == JobState::Succeeded;
    let message = if op == CronOperation::RunNow && succeeded {
        let logs = state
            .jobs_store
            .lock()
            .await
            .list_logs(&job.id)
            .unwrap_or_default();
        logs.last()
            .map(|entry| entry.text.clone())
            .or_else(|| Some("succeeded".to_string()))
    } else {
        job.error_text.clone()
    };
    let now = Utc::now().to_rfc3339();
    state
        .control_store
        .lock()
        .await
        .complete_cron_operation(request.request_id, op, succeeded, message.as_deref(), &now)
        .map_err(|_| node_jobs::internal_error())?;
    Ok(())
}

fn require_idempotency_key(headers: &HeaderMap) -> Result<&str, ErrorResponse> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty())
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
            message: "Cron job not found",
        },
    }
}

fn map_store_error(error: StoreError) -> ErrorResponse {
    match error {
        StoreError::NotFound(_) => not_found(),
        StoreError::Conflict { .. } => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Idempotency-Key reused with different cron payload",
            },
        },
        _ => node_jobs::internal_error(),
    }
}

#[cfg(test)]
mod tests {
    use crate::app;
    use crate::AppState;
    use axum::{
        body::Body,
        http::{header, Request as HttpRequest, StatusCode},
    };
    use fusionpanel_control_store::ControlStore;
    use fusionpanel_jobs::JobStore;
    use identity::{Role, RoleBinding, Scope, Session, SqliteIdentityStore, TenantId, User};
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;
    use uuid::Uuid;

    fn test_state() -> AppState {
        AppState {
            admin_token: Arc::new("test-admin-token".to_string()),
            identity_store: Arc::new(Mutex::new(SqliteIdentityStore::in_memory().unwrap())),
            control_store: Arc::new(Mutex::new(ControlStore::new_in_memory().unwrap())),
            jobs_store: Arc::new(Mutex::new(JobStore::in_memory().unwrap())),
            frontend_dir: None,
            metrics_store: crate::metrics::shared_store(),
            alerts_store: crate::alerts::test_store(),
        }
    }

    async fn create_session(state: &AppState, tenant_id: Uuid, role: Role) -> String {
        let user = User::new(
            format!("cron-test-{}", Uuid::new_v4()),
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
    async fn cron_routes_enforce_tenant_isolation_and_schedule_validation() {
        let state = test_state();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let sess_a = create_session(&state, tenant_a, Role::tenant_admin()).await;
        let sess_b = create_session(&state, tenant_b, Role::tenant_admin()).await;
        let router = app(state);

        let resp = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/cron")
                    .header(header::AUTHORIZATION, format!("Bearer {sess_a}"))
                    .header("idempotency-key", "c1")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        r#"{"name":"ping","schedule":"*/5 * * * *","command":"echo ok"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::ACCEPTED);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let cron_id = json["cron_job"]["id"].as_str().unwrap();

        let foreign = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/cron/{cron_id}/run"))
                    .header(header::AUTHORIZATION, format!("Bearer {sess_b}"))
                    .header("idempotency-key", "c2")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(foreign.status(), StatusCode::NOT_FOUND);
    }
}

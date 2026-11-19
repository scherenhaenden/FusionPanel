#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State},
    http::{header::AUTHORIZATION, HeaderMap, StatusCode},
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use fusionpanel_agent_protocol::{Operation, Request};
use fusionpanel_domain::NodeId;
use fusionpanel_jobs::{Job, JobState};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;
use uuid::Uuid;

use super::{validate_session_id, AppState, ErrorDetail, ErrorResponse};

const NODE_ONLINE_WINDOW_SECONDS: i64 = 90;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RegisterNodeRequest {
    name: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct RegisterNodeResponse {
    node_id: String,
    token: String,
}

#[derive(Debug, Serialize)]
struct NodeView {
    node_id: String,
    name: String,
    state: &'static str,
    created_at: String,
    last_heartbeat: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct EnqueueJobRequest {
    operation: String,
    payload: Value,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClaimRequest {
    node_id: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AppendLogRequest {
    node_id: String,
    stream: String,
    text: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompleteJobRequest {
    node_id: String,
    state: JobState,
    #[serde(default)]
    error_text: Option<String>,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/nodes", get(list_nodes))
        .route("/v1/nodes/register", post(register_node))
        .route("/v1/nodes/{id}/heartbeat", post(heartbeat_node))
        .route("/v1/jobs", get(list_jobs).post(enqueue_job))
        .route("/v1/jobs/claim", post(claim_job))
        .route("/v1/jobs/{id}", get(get_job))
        .route("/v1/jobs/{id}/logs", get(get_job_logs).post(append_job_log))
        .route("/v1/jobs/{id}/complete", post(complete_job))
}

async fn register_node(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<RegisterNodeRequest>,
) -> Result<(StatusCode, Json<RegisterNodeResponse>), ErrorResponse> {
    if !authorized_admin(&state, &headers) {
        return Err(unauthorized());
    }
    let name = request.name.trim();
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err(bad_request());
    }

    let proposed_node_id = NodeId::new().to_string();
    let proposed_id = Uuid::parse_str(&proposed_node_id).map_err(|_| internal_error())?;
    // Two UUIDs provide a high-entropy bearer token without requiring a token
    // to be stored in plaintext in SQLite.
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let token_hash = hash_token(&token);
    let now = Utc::now().to_rfc3339();
    let node_id = state
        .control_store
        .lock()
        .await
        .register_node(proposed_id, name, &token_hash, &now)
        .map_err(|_| internal_error())?;

    Ok((
        StatusCode::CREATED,
        Json(RegisterNodeResponse {
            node_id: node_id.to_string(),
            token,
        }),
    ))
}

async fn heartbeat_node(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ErrorResponse> {
    let node_id = parse_uuid(&id)?;
    let token = bearer_token(&headers).ok_or_else(unauthorized)?;
    authenticate_node(&state, node_id, token).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_nodes(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    if !authorized_operator(&state, &headers).await {
        return Err(unauthorized());
    }
    let now = Utc::now();
    let nodes = state
        .control_store
        .lock()
        .await
        .list_nodes()
        .map_err(|_| internal_error())?
        .into_iter()
        .map(|node| {
            let last_heartbeat = chrono::DateTime::parse_from_rfc3339(&node.last_heartbeat)
                .map(|time| time.with_timezone(&Utc));
            let online = last_heartbeat
                .map(|time| {
                    let age = (now - time).num_seconds();
                    (0..=NODE_ONLINE_WINDOW_SECONDS).contains(&age)
                })
                .unwrap_or(false);
            NodeView {
                node_id: node.node_id.to_string(),
                name: node.name,
                state: if online { "online" } else { "offline" },
                created_at: node.created_at,
                last_heartbeat: node.last_heartbeat,
            }
        })
        .collect::<Vec<_>>();
    Ok(Json(json!({ "nodes": nodes })))
}

async fn enqueue_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<EnqueueJobRequest>,
) -> Result<(StatusCode, Json<Value>), ErrorResponse> {
    if !authorized_operator(&state, &headers).await {
        return Err(unauthorized());
    }
    if matches!(
        body.operation.as_str(),
        "site.apply" | "site.delete" | "tls.apply" | "tls.delete"
    ) {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Use the Websites API for website operations",
            },
        });
    }
    if matches!(
        body.operation.as_str(),
        "database.apply" | "database.delete" | "database.user.create"
    ) {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Use the Databases API for database operations",
            },
        });
    }
    if matches!(body.operation.as_str(), "backup.create" | "backup.restore") {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Use the Backups API for backup operations",
            },
        });
    }
    if matches!(
        body.operation.as_str(),
        "dns.zone.apply" | "dns.zone.delete" | "dns.probe"
    ) {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Use the DNS API for DNS operations",
            },
        });
    }
    if matches!(
        body.operation.as_str(),
        "cron.apply" | "cron.delete" | "cron.run_now"
    ) {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Use the Cron API for cron operations",
            },
        });
    }
    let key = headers
        .get("idempotency-key")
        .and_then(|value| value.to_str().ok())
        .filter(|value| !value.trim().is_empty() && value.len() <= 255)
        .ok_or_else(bad_request)?;

    let fingerprint = json!({
        "operation": body.operation,
        "payload": body.payload,
    });
    let operation: Operation =
        serde_json::from_value(fingerprint.clone()).map_err(|_| bad_request())?;
    let request = Request {
        version: 1,
        request_id: Uuid::new_v4(),
        operation,
    };
    request.validate().map_err(|_| bad_request())?;
    let payload = serde_json::to_value(request).map_err(|_| internal_error())?;
    let (id, created) = state
        .jobs_store
        .lock()
        .await
        .create_idempotent("agent_operation", &payload, key, &fingerprint)
        .map_err(map_job_error)?;
    let job = state
        .jobs_store
        .lock()
        .await
        .get(&id)
        .map_err(map_job_error)?;
    let status = if created {
        StatusCode::CREATED
    } else {
        StatusCode::OK
    };
    Ok((status, Json(json!({ "job": job }))))
}

async fn list_jobs(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    if !authorized_operator(&state, &headers).await {
        return Err(unauthorized());
    }
    let jobs: Vec<Job> = state
        .jobs_store
        .lock()
        .await
        .list()
        .map_err(map_job_error)?
        .into_iter()
        .map(sanitize_job_for_operator)
        .collect();
    Ok(Json(json!({ "jobs": jobs })))
}

async fn get_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    if !authorized_operator(&state, &headers).await {
        return Err(unauthorized());
    }
    let job = state
        .jobs_store
        .lock()
        .await
        .get(&id)
        .map_err(map_job_error)?;
    Ok(Json(json!({ "job": sanitize_job_for_operator(job) })))
}

pub(super) fn sanitize_job_for_operator(mut job: Job) -> Job {
    if let Some(payload_obj) = job
        .payload
        .get_mut("payload")
        .and_then(Value::as_object_mut)
    {
        if payload_obj.contains_key("password") {
            payload_obj.insert(
                "password".to_string(),
                Value::String("[REDACTED]".to_string()),
            );
        }
        if let Some(source) = payload_obj.get_mut("source").and_then(Value::as_object_mut) {
            if source.contains_key("private_key_pem") {
                source.insert(
                    "private_key_pem".to_string(),
                    Value::String("[REDACTED]".to_string()),
                );
            }
        }
        if let Some(dest) = payload_obj
            .get_mut("destination")
            .and_then(Value::as_object_mut)
        {
            if dest.contains_key("secret_key") {
                dest.insert(
                    "secret_key".to_string(),
                    Value::String("[REDACTED]".to_string()),
                );
            }
        }
    }
    job
}

async fn get_job_logs(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<Json<Value>, ErrorResponse> {
    if !authorized_operator(&state, &headers).await {
        return Err(unauthorized());
    }
    let logs = state
        .jobs_store
        .lock()
        .await
        .list_logs(&id)
        .map_err(map_job_error)?;
    Ok(Json(json!({ "logs": logs })))
}

async fn claim_job(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(request): Json<ClaimRequest>,
) -> Result<Json<Value>, ErrorResponse> {
    let node_id = parse_uuid(&request.node_id)?;
    let token = bearer_token(&headers).ok_or_else(unauthorized)?;
    authenticate_node(&state, node_id, token).await?;
    let job = state
        .jobs_store
        .lock()
        .await
        .claim(&node_id.to_string())
        .map_err(map_job_error)?;
    Ok(Json(json!({ "job": job })))
}

async fn append_job_log(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<AppendLogRequest>,
) -> Result<StatusCode, ErrorResponse> {
    let node_id = parse_uuid(&request.node_id)?;
    let token = bearer_token(&headers).ok_or_else(unauthorized)?;
    authenticate_node(&state, node_id, token).await?;
    let text = redact_known_secrets(&request.text, &[token, state.admin_token.as_str()]);
    state
        .jobs_store
        .lock()
        .await
        .append_log(&id, &node_id.to_string(), &request.stream, &text)
        .map_err(map_job_error)?;
    Ok(StatusCode::CREATED)
}

async fn complete_job(
    State(state): State<AppState>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<CompleteJobRequest>,
) -> Result<StatusCode, ErrorResponse> {
    let node_id = parse_uuid(&request.node_id)?;
    let token = bearer_token(&headers).ok_or_else(unauthorized)?;
    authenticate_node(&state, node_id, token).await?;
    if !matches!(
        request.state,
        JobState::Succeeded | JobState::Failed | JobState::Cancelled
    ) {
        return Err(bad_request());
    }
    let error_text = request
        .error_text
        .map(|text| redact_known_secrets(&text, &[token, state.admin_token.as_str()]));
    let node_id = node_id.to_string();
    {
        let mut jobs = state.jobs_store.lock().await;
        let current = jobs.get(&id).map_err(map_job_error)?;
        let is_same_terminal_retry = current.state == request.state
            && current.worker_id.as_deref() == Some(node_id.as_str())
            && matches!(
                current.state,
                JobState::Succeeded | JobState::Failed | JobState::Cancelled
            );
        if !is_same_terminal_retry {
            jobs.transition_for_worker(&id, &node_id, request.state, error_text)
                .map_err(map_job_error)?;
        }
    }
    let completed = state
        .jobs_store
        .lock()
        .await
        .get(&id)
        .map_err(map_job_error)?;
    super::websites::record_completion(&state, &completed).await?;
    super::databases::record_completion(&state, &completed).await?;
    super::backups::record_completion(&state, &completed).await?;
    super::dns::record_completion(&state, &completed).await?;
    super::cron::record_completion(&state, &completed).await?;
    Ok(StatusCode::NO_CONTENT)
}

pub(super) async fn authenticate_node(
    state: &AppState,
    node_id: Uuid,
    token: &str,
) -> Result<(), ErrorResponse> {
    let presented = hash_token(token);
    let stored = state
        .control_store
        .lock()
        .await
        .node_token_hash(node_id)
        .map_err(|_| internal_error())?
        .ok_or_else(unauthorized)?;
    if bool::from(presented.as_bytes().ct_eq(stored.as_bytes())) {
        state
            .control_store
            .lock()
            .await
            .touch_node(node_id, &Utc::now().to_rfc3339())
            .map_err(|_| internal_error())?;
        Ok(())
    } else {
        Err(unauthorized())
    }
}

pub(super) async fn authorized_operator(state: &AppState, headers: &HeaderMap) -> bool {
    if authorized_admin(state, headers) {
        return true;
    }
    let bearer_session = bearer_token(headers);
    let session_id = bearer_session
        .map(str::to_owned)
        .or_else(|| super::extract_session_from_cookies(headers));
    match session_id {
        Some(session_id) => validate_session_id(&state.identity_store, &session_id).await,
        None => false,
    }
}

pub(super) fn authorized_admin(state: &AppState, headers: &HeaderMap) -> bool {
    let Some(token) = bearer_token(headers) else {
        return false;
    };
    token.len() == state.admin_token.len()
        && bool::from(token.as_bytes().ct_eq(state.admin_token.as_bytes()))
}

pub(super) fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)
        .and_then(|header| header.to_str().ok())
        .and_then(|header| header.strip_prefix("Bearer "))
        .filter(|token| !token.is_empty())
}

pub(super) fn hash_token(token: &str) -> String {
    format!("{:x}", Sha256::digest(token.as_bytes()))
}

fn parse_uuid(value: &str) -> Result<Uuid, ErrorResponse> {
    Uuid::parse_str(value).map_err(|_| bad_request())
}

fn redact_known_secrets(value: &str, secrets: &[&str]) -> String {
    secrets
        .iter()
        .filter(|secret| !secret.is_empty())
        .fold(value.to_string(), |text, secret| {
            text.replace(secret, "[REDACTED]")
        })
}

pub(super) fn map_job_error(error: fusionpanel_jobs::JobStoreError) -> ErrorResponse {
    match error {
        fusionpanel_jobs::JobStoreError::NotFound(_) => ErrorResponse {
            error: ErrorDetail {
                code: "not_found",
                message: "Not found",
            },
        },
        fusionpanel_jobs::JobStoreError::IdempotencyConflict
        | fusionpanel_jobs::JobStoreError::InvalidTransition { .. } => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Job request conflicts with existing state",
            },
        },
        fusionpanel_jobs::JobStoreError::WrongWorker => unauthorized(),
        fusionpanel_jobs::JobStoreError::InvalidLogStream
        | fusionpanel_jobs::JobStoreError::ErrorTextTooLong { .. }
        | fusionpanel_jobs::JobStoreError::LogTextTooLong { .. }
        | fusionpanel_jobs::JobStoreError::LogLimitExceeded { .. } => bad_request(),
        _ => internal_error(),
    }
}

pub(super) fn unauthorized() -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "unauthorized",
            message: "Authentication required",
        },
    }
}

pub(super) fn bad_request() -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "bad_request",
            message: "Invalid request",
        },
    }
}

pub(super) fn internal_error() -> ErrorResponse {
    ErrorResponse {
        error: ErrorDetail {
            code: "internal_error",
            message: "Internal server error",
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use std::sync::Arc;
    use tokio::sync::Mutex;
    use tower::ServiceExt;

    fn test_state() -> AppState {
        AppState {
            admin_token: Arc::new("test-admin-token".to_string()),
            identity_store: Arc::new(Mutex::new(
                identity::SqliteIdentityStore::in_memory().unwrap(),
            )),
            control_store: Arc::new(Mutex::new(
                fusionpanel_control_store::ControlStore::new_in_memory().unwrap(),
            )),
            jobs_store: Arc::new(Mutex::new(fusionpanel_jobs::JobStore::in_memory().unwrap())),
            frontend_dir: None,
            metrics_store: crate::metrics::shared_store(),
            alerts_store: crate::alerts::test_store(),
        }
    }

    #[tokio::test]
    async fn node_register_heartbeat_and_list_never_exposes_token() {
        let state = test_state();
        let router = super::super::app(state.clone());
        let response = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/nodes/register")
                    .header(AUTHORIZATION, "Bearer test-admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"local-node"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CREATED);
        let registration: RegisterNodeResponse =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert!(!registration.token.is_empty());

        let heartbeat = super::super::app(state.clone());
        let response = heartbeat
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/nodes/{}/heartbeat", registration.node_id))
                    .header(AUTHORIZATION, format!("Bearer {}", registration.token))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);

        let list = super::super::app(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/v1/nodes")
                    .header(AUTHORIZATION, "Bearer test-admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(list.status(), StatusCode::OK);
        let list_body = list.into_body().collect().await.unwrap().to_bytes();
        let list_json: Value = serde_json::from_slice(&list_body).unwrap();
        assert_eq!(list_json["nodes"][0]["name"], "local-node");
        assert!(serde_json::to_string(&list_json)
            .unwrap()
            .find(&registration.token)
            .is_none());

        // Restarting an agent registers by its configured name; keep the node
        // identity stable while rotating its one-time bearer token.
        let reregistered = super::super::app(state.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/nodes/register")
                    .header(AUTHORIZATION, "Bearer test-admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"local-node"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(reregistered.status(), StatusCode::CREATED);
        let reregistered: RegisterNodeResponse =
            serde_json::from_slice(&reregistered.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(reregistered.node_id, registration.node_id);
        assert_ne!(reregistered.token, registration.token);

        let unauthorized_heartbeat = super::super::app(state)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/nodes/{}/heartbeat", registration.node_id))
                    .header(AUTHORIZATION, "Bearer wrong-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized_heartbeat.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn jobs_require_auth_and_idempotency_key() {
        let router = super::super::app(test_state());
        let unauthenticated = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/v1/jobs")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
        let request = Request::builder()
            .method("POST")
            .uri("/v1/jobs")
            .header(AUTHORIZATION, "Bearer test-admin-token")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"operation":"system.info","payload":{}}"#))
            .unwrap();
        let response = router.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[tokio::test]
    async fn generic_job_enqueue_rejects_website_lifecycle_operations() {
        let router = super::super::app(test_state());
        for (key, body) in [
            (
                "generic-site-apply",
                r#"{"operation":"site.apply","payload":{"site_id":"d4c61b93-8a7a-492a-b252-3fc286eb5634","domain":"example.com","spec":{"kind":"static","document_root":"/srv/fusionpanel/sites/example.com"}}}"#,
            ),
            (
                "generic-site-delete",
                r#"{"operation":"site.delete","payload":{"site_id":"d4c61b93-8a7a-492a-b252-3fc286eb5634","domain":"example.com"}}"#,
            ),
        ] {
            let response = router
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri("/v1/jobs")
                        .header(AUTHORIZATION, "Bearer test-admin-token")
                        .header("idempotency-key", key)
                        .header("content-type", "application/json")
                        .body(Body::from(body))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{key}");
            let error: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(
                error["error"]["message"], "Use the Websites API for website operations",
                "{key}"
            );
        }

        let system_info = router
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/jobs")
                    .header(AUTHORIZATION, "Bearer test-admin-token")
                    .header("idempotency-key", "generic-system-info")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"operation":"system.info","payload":{}}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(system_info.status(), StatusCode::CREATED);
    }

    #[tokio::test]
    async fn jobs_enqueue_idempotently_claim_log_and_complete() {
        let state = test_state();
        let router = super::super::app(state.clone());
        let registration = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/nodes/register")
                    .header(AUTHORIZATION, "Bearer test-admin-token")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"name":"worker"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        let node: RegisterNodeResponse =
            serde_json::from_slice(&registration.into_body().collect().await.unwrap().to_bytes())
                .unwrap();

        let enqueue = || {
            Request::builder()
                .method("POST")
                .uri("/v1/jobs")
                .header(AUTHORIZATION, "Bearer test-admin-token")
                .header("idempotency-key", "request-1")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"operation":"system.info","payload":{}}"#))
                .unwrap()
        };
        let first = router.clone().oneshot(enqueue()).await.unwrap();
        assert_eq!(first.status(), StatusCode::CREATED);
        let first_json: Value =
            serde_json::from_slice(&first.into_body().collect().await.unwrap().to_bytes()).unwrap();
        let second = router.clone().oneshot(enqueue()).await.unwrap();
        assert_eq!(second.status(), StatusCode::OK);
        let second_json: Value =
            serde_json::from_slice(&second.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(first_json["job"]["id"], second_json["job"]["id"]);
        let job_id = first_json["job"]["id"].as_str().unwrap();

        let claim = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/jobs/claim")
                    .header(AUTHORIZATION, format!("Bearer {}", node.token))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(r#"{{"node_id":"{}"}}"#, node.node_id)))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claim.status(), StatusCode::OK);
        let claim_json: Value =
            serde_json::from_slice(&claim.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(claim_json["job"]["id"], job_id);
        assert_eq!(claim_json["job"]["state"], "running");

        let log_response = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/jobs/{job_id}/logs"))
                    .header(AUTHORIZATION, format!("Bearer {}", node.token))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"node_id":"{}","stream":"stdout","text":"{}"}}"#,
                        node.node_id, node.token
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(log_response.status(), StatusCode::CREATED);

        let complete = router
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/jobs/{job_id}/complete"))
                    .header(AUTHORIZATION, format!("Bearer {}", node.token))
                    .header("content-type", "application/json")
                    .body(Body::from(format!(
                        r#"{{"node_id":"{}","state":"succeeded"}}"#,
                        node.node_id
                    )))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(complete.status(), StatusCode::NO_CONTENT);

        let logs = router
            .oneshot(
                Request::builder()
                    .uri(format!("/v1/jobs/{job_id}/logs"))
                    .header(AUTHORIZATION, "Bearer test-admin-token")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let logs_json: Value =
            serde_json::from_slice(&logs.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(logs_json["logs"][0]["text"], "[REDACTED]");
    }
}

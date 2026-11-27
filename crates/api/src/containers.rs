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
    normalize_container_ports, ContainerApplyPayload, ContainerDeletePayload,
    ContainerLifecyclePayload, ContainerPortMapping, ContainerPortProtocol,
    ContainerResourceLimits, ContainerRestartPolicy, Operation, Request,
};
use fusionpanel_control_store::{
    ContainerOperation, ContainerPortSpec, ContainerResourceSpec, NewContainerSpec, StoreError,
};
use fusionpanel_jobs::{Job, JobState};
use identity::Permission;
use serde::Deserialize;
use serde_json::json;
use std::collections::BTreeMap;
use uuid::Uuid;

fn default_container_resources() -> ContainerResourceLimits {
    ContainerResourceLimits {
        cpu_millicores: 500,
        memory_bytes: 256 * 1024 * 1024,
        pids_limit: Some(256),
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateContainerRequest {
    name: String,
    image: String,
    #[serde(default)]
    app_id: Option<String>,
    #[serde(default)]
    node_id: Option<Uuid>,
    #[serde(default)]
    ports: Vec<ContainerPortMapping>,
    #[serde(default)]
    port: Option<u16>,
    #[serde(default = "default_container_resources")]
    resources: ContainerResourceLimits,
    #[serde(default)]
    env: BTreeMap<String, String>,
    #[serde(default)]
    command: Vec<String>,
    #[serde(default)]
    restart_policy: ContainerRestartPolicy,
}

pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route("/v1/containers/catalog", get(list_container_catalog))
        .route("/api/v1/containers/catalog", get(list_container_catalog))
        .route(
            "/v1/containers",
            get(list_containers).post(create_container),
        )
        .route(
            "/api/v1/containers",
            get(list_containers).post(create_container),
        )
        .route(
            "/v1/containers/{id}",
            get(get_container).delete(delete_container),
        )
        .route(
            "/api/v1/containers/{id}",
            get(get_container).delete(delete_container),
        )
        .route("/v1/containers/{id}/logs", get(get_container_logs))
        .route("/api/v1/containers/{id}/logs", get(get_container_logs))
        .route("/v1/containers/{id}/start", post(start_container))
        .route("/api/v1/containers/{id}/start", post(start_container))
        .route("/v1/containers/{id}/stop", post(stop_container))
        .route("/api/v1/containers/{id}/stop", post(stop_container))
        .route("/v1/containers/{id}/restart", post(restart_container))
        .route("/api/v1/containers/{id}/restart", post(restart_container))
}

async fn list_container_catalog(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteRead, None)?;
    Ok(Json(json!({ "apps": [] })))
}

async fn get_container_logs(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteRead, None)?;
    let container_id = parse_uuid(&id)?;
    let _container = state
        .control_store
        .lock()
        .await
        .get_container_for_tenant(container_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "logs": [], "truncated": false })))
}

async fn list_containers(
    State(state): State<AppState>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteRead, None)?;
    let containers = state
        .control_store
        .lock()
        .await
        .list_containers_for_tenant(context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?;
    Ok(Json(json!({ "containers": containers })))
}

async fn get_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<serde_json::Value>, ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteRead, None)?;
    let container_id = parse_uuid(&id)?;
    let container = state
        .control_store
        .lock()
        .await
        .get_container_for_tenant(container_id, context.tenant_id.to_uuid())
        .map_err(|_| node_jobs::internal_error())?
        .ok_or_else(not_found)?;
    Ok(Json(json!({ "container": container })))
}

async fn create_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateContainerRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteCreate, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let tenant_id = context.tenant_id.to_uuid();

    let _app_id = body.app_id.as_deref();
    let name = body.name.trim().to_string();
    let image = body.image.trim().to_string();
    let raw_ports = if body.ports.is_empty() {
        if let Some(host_port) = body.port {
            vec![ContainerPortMapping {
                host_port,
                container_port: 8080,
                protocol: ContainerPortProtocol::Tcp,
            }]
        } else {
            Vec::new()
        }
    } else {
        body.ports.clone()
    };
    let normalized_ports = normalize_container_ports(&raw_ports)
        .map_err(|_| bad_request("Invalid or duplicate container port mappings"))?;

    if let Some(node_id) = body.node_id {
        if node_id.is_nil() {
            return Err(bad_request("Invalid node_id"));
        }
        let exists = state
            .control_store
            .lock()
            .await
            .node_exists(node_id)
            .map_err(|_| node_jobs::internal_error())?;
        if !exists {
            return Err(ErrorResponse {
                error: ErrorDetail {
                    code: "not_found",
                    message: "Node not found",
                },
            });
        }
    }

    let probe = ContainerApplyPayload {
        container_id: Uuid::new_v4(),
        tenant_id,
        name: name.clone(),
        image: image.clone(),
        ports: normalized_ports.clone(),
        resources: body.resources,
        env: body.env.clone(),
        command: body.command.clone(),
        restart_policy: body.restart_policy,
    };
    probe.validate().map_err(|err| {
        use fusionpanel_agent_protocol::ValidationError as VE;
        match err {
            VE::InvalidContainerImage => {
                bad_request("Container image must be a valid sha256 digest-pinned reference")
            }
            VE::InvalidContainerPorts => {
                bad_request("Invalid or duplicate container port mappings")
            }
            VE::InvalidContainerResources => {
                bad_request("Container resource limits are out of bounds")
            }
            VE::InvalidContainerName => bad_request("Invalid container name"),
            VE::InvalidContainerEnv => bad_request("Invalid container environment variables"),
            VE::InvalidContainerCommand => bad_request("Invalid container command"),
            _ => bad_request("Invalid container specification"),
        }
    })?;

    let store_ports: Vec<ContainerPortSpec> = normalized_ports
        .iter()
        .map(|p| ContainerPortSpec {
            host_port: p.host_port,
            container_port: p.container_port,
            protocol: p.protocol.as_str().to_string(),
        })
        .collect();
    let store_resources = ContainerResourceSpec {
        cpu_millicores: body.resources.cpu_millicores,
        memory_bytes: body.resources.memory_bytes,
        pids_limit: body.resources.pids_limit,
    };

    let fingerprint_json = json!({
        "op": "container.apply",
        "tenant_id": tenant_id,
        "node_id": body.node_id,
        "name": &name,
        "image": &image,
        "ports": &normalized_ports,
        "resources": &body.resources,
        "env": &body.env,
        "command": &body.command,
        "restart_policy": body.restart_policy,
    });
    let fingerprint =
        serde_json::to_string(&fingerprint_json).map_err(|_| node_jobs::internal_error())?;
    let scoped_key = format!("container-create:{tenant_id}:{idempotency_key}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_container_create(
            NewContainerSpec {
                tenant_id,
                node_id: body.node_id,
                name: &name,
                image: &image,
                ports: &store_ports,
                resources: &store_resources,
                env: &body.env,
                command: &body.command,
                restart_policy: body.restart_policy.as_str(),
            },
            &scoped_key,
            &fingerprint,
            &now,
        )
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.existing_job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::ContainerApply(ContainerApplyPayload {
                container_id: reservation.container.id,
                tenant_id,
                name: reservation.container.name.clone(),
                image: reservation.container.image.clone(),
                ports: normalized_ports,
                resources: body.resources,
                env: body.env,
                command: body.command,
                restart_policy: body.restart_policy,
            }),
        };
        enqueue_container_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "container": reservation.container,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn start_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    lifecycle_container(state, headers, id, ContainerOperation::Start).await
}

async fn stop_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    lifecycle_container(state, headers, id, ContainerOperation::Stop).await
}

async fn restart_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    lifecycle_container(state, headers, id, ContainerOperation::Restart).await
}

async fn delete_container(
    State(state): State<AppState>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteDelete, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let container_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    let scoped_key = format!("container-delete:{tenant_id}:{container_id}:{idempotency_key}");
    let fingerprint = format!("container.delete:{tenant_id}:{container_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_container_lifecycle(
            container_id,
            tenant_id,
            ContainerOperation::Delete,
            &scoped_key,
            &fingerprint,
            &now,
        )
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.existing_job_id).await?
    } else {
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: Operation::ContainerDelete(ContainerDeletePayload {
                container_id,
                tenant_id,
                name: reservation.container.name.clone(),
            }),
        };
        enqueue_container_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "container": reservation.container,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn lifecycle_container(
    state: AppState,
    headers: HeaderMap,
    id: String,
    operation: ContainerOperation,
) -> Result<(StatusCode, Json<serde_json::Value>), ErrorResponse> {
    let context = website_authorization_context(&state, &headers).await?;
    require_website_permission(&context, Permission::SiteUpdate, None)?;
    let idempotency_key = require_idempotency_key(&headers)?;
    let container_id = parse_uuid(&id)?;
    let tenant_id = context.tenant_id.to_uuid();
    let op_str = operation.as_str();
    let scoped_key = format!("{op_str}:{tenant_id}:{container_id}:{idempotency_key}");
    let fingerprint = format!("{op_str}:{tenant_id}:{container_id}");
    let now = Utc::now().to_rfc3339();

    let reservation = state
        .control_store
        .lock()
        .await
        .reserve_container_lifecycle(
            container_id,
            tenant_id,
            operation,
            &scoped_key,
            &fingerprint,
            &now,
        )
        .map_err(map_store_error)?;

    let job = if reservation.idempotent_replay {
        load_existing_job(&state, reservation.existing_job_id).await?
    } else {
        let payload = ContainerLifecyclePayload {
            container_id,
            tenant_id,
            name: reservation.container.name.clone(),
        };
        let op = match operation {
            ContainerOperation::Start => Operation::ContainerStart(payload),
            ContainerOperation::Stop => Operation::ContainerStop(payload),
            ContainerOperation::Restart => Operation::ContainerRestart(payload),
            _ => return Err(node_jobs::internal_error()),
        };
        let req = Request {
            version: 1,
            request_id: reservation.request_id,
            operation: op,
        };
        enqueue_container_job(&state, &scoped_key, req).await?
    };

    Ok((
        StatusCode::ACCEPTED,
        Json(json!({
            "container": reservation.container,
            "job": node_jobs::sanitize_job_for_operator(job),
        })),
    ))
}

async fn enqueue_container_job(
    state: &AppState,
    scoped_key: &str,
    req: Request,
) -> Result<Job, ErrorResponse> {
    req.validate()
        .map_err(|_| bad_request("Invalid container operation payload"))?;
    let payload_value = serde_json::to_value(&req).map_err(|_| node_jobs::internal_error())?;
    let job_key = format!("container-job:{scoped_key}");
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
        .set_container_operation_job_id(scoped_key, job_uuid)
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

pub(super) async fn record_completion(state: &AppState, job: &Job) -> Result<(), ErrorResponse> {
    let Ok(request) = serde_json::from_value::<Request>(job.payload.clone()) else {
        return Ok(());
    };
    let op = match &request.operation {
        Operation::ContainerApply(_) => ContainerOperation::Apply,
        Operation::ContainerStart(_) => ContainerOperation::Start,
        Operation::ContainerStop(_) => ContainerOperation::Stop,
        Operation::ContainerRestart(_) => ContainerOperation::Restart,
        Operation::ContainerDelete(_) => ContainerOperation::Delete,
        _ => return Ok(()),
    };
    let worker_node_id = job
        .worker_id
        .as_deref()
        .and_then(|s| Uuid::parse_str(s).ok());
    let succeeded = job.state == JobState::Succeeded;
    let now = Utc::now().to_rfc3339();
    state
        .control_store
        .lock()
        .await
        .complete_container_operation(
            request.request_id,
            op,
            worker_node_id,
            succeeded,
            job.error_text.as_deref(),
            &now,
        )
        .map_err(map_store_error)?;
    Ok(())
}

fn require_idempotency_key(headers: &HeaderMap) -> Result<&str, ErrorResponse> {
    headers
        .get("idempotency-key")
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|v| !v.is_empty() && v.len() <= 255 && v.is_ascii())
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
            message: "Container not found",
        },
    }
}

fn map_store_error(error: StoreError) -> ErrorResponse {
    match error {
        StoreError::ContainerNotFound(_) | StoreError::NotFound(_) => not_found(),
        StoreError::IdempotencyConflict => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Idempotency-Key reused with different container payload",
            },
        },
        StoreError::ContainerOperationInProgress => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Container operation is already in progress",
            },
        },
        StoreError::ContainerConflict(_) | StoreError::Conflict { .. } => ErrorResponse {
            error: ErrorDetail {
                code: "conflict",
                message: "Container request conflicts with existing state",
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
    use serde_json::json;
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
        }
    }

    async fn create_session(state: &AppState, tenant_id: Uuid, role: Role) -> String {
        let user = User::new(
            format!("container-test-{}", Uuid::new_v4()),
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

    async fn register_node(router: &axum::Router) -> (String, String) {
        let resp = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/nodes/register")
                    .header(header::AUTHORIZATION, "Bearer test-admin-token")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(r#"{"name":"worker-1"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
        let body = axum::body::to_bytes(resp.into_body(), 64 * 1024)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        (
            json["node_id"].as_str().unwrap().to_string(),
            json["token"].as_str().unwrap().to_string(),
        )
    }

    #[tokio::test]
    async fn container_routes_enforce_rbac_tenant_isolation_pinned_image_and_idempotency() {
        let state = test_state();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let admin_a = create_session(&state, tenant_a, Role::tenant_admin()).await;
        let viewer_a = create_session(&state, tenant_a, Role::viewer()).await;
        let admin_b = create_session(&state, tenant_b, Role::tenant_admin()).await;
        let router = app(state);

        let (node_id, node_token) = register_node(&router).await;
        let digest = "a".repeat(64);
        let valid_body = json!({
            "name": "api-svc",
            "image": format!("ghcr.io/fusionpanel/api:1.0@sha256:{digest}"),
            "node_id": node_id,
            "ports": [
                {"host_port": 8443, "container_port": 443, "protocol": "tcp"},
                {"host_port": 8080, "container_port": 80}
            ],
            "resources": {
                "cpu_millicores": 500,
                "memory_bytes": 268435456u64,
                "pids_limit": 128
            }
        });

        // Viewer cannot create container (RBAC)
        let forbidden = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/containers")
                    .header(header::AUTHORIZATION, format!("Bearer {viewer_a}"))
                    .header("idempotency-key", "k-viewer")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(valid_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);

        // Unpinned image rejected
        let unpinned = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/containers")
                    .header(header::AUTHORIZATION, format!("Bearer {admin_a}"))
                    .header("idempotency-key", "k-unpinned")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "name": "bad-img",
                            "image": "nginx:latest",
                            "resources": {"cpu_millicores": 500, "memory_bytes": 268435456u64}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unpinned.status(), StatusCode::BAD_REQUEST);

        // Create succeeds on /api/v1/containers mirror
        let created = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/api/v1/containers")
                    .header(header::AUTHORIZATION, format!("Bearer {admin_a}"))
                    .header("idempotency-key", "k-create-1")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(valid_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(created.status(), StatusCode::ACCEPTED);
        let created_json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(created.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        let container_id = created_json["container"]["id"].as_str().unwrap();
        let job_id = created_json["job"]["id"].as_str().unwrap();
        // Ports normalized in ascending host_port order
        assert_eq!(created_json["container"]["ports"][0]["host_port"], 8080);
        assert_eq!(created_json["container"]["ports"][1]["host_port"], 8443);

        // Exact idempotent replay returns same container + job
        let replay = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/containers")
                    .header(header::AUTHORIZATION, format!("Bearer {admin_a}"))
                    .header("idempotency-key", "k-create-1")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(valid_body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(replay.status(), StatusCode::ACCEPTED);
        let replay_json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(replay.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(replay_json["container"]["id"], container_id);
        assert_eq!(replay_json["job"]["id"], job_id);

        // Conflicting replay returns 409 conflict
        let conflict = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/containers")
                    .header(header::AUTHORIZATION, format!("Bearer {admin_a}"))
                    .header("idempotency-key", "k-create-1")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "name": "other-svc",
                            "image": format!("ghcr.io/fusionpanel/api:1.0@sha256:{digest}"),
                            "resources": {"cpu_millicores": 500, "memory_bytes": 268435456u64}
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(conflict.status(), StatusCode::CONFLICT);

        // Tenant B cannot see or stop tenant A's container
        let foreign_get = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/containers/{container_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {admin_b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(foreign_get.status(), StatusCode::NOT_FOUND);

        // Node claims and fails the job -> honest Failed state with error_text
        let claim = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri("/v1/jobs/claim")
                    .header(header::AUTHORIZATION, format!("Bearer {node_token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(json!({ "node_id": node_id }).to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(claim.status(), StatusCode::OK);

        let complete_fail = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("POST")
                    .uri(format!("/v1/jobs/{job_id}/complete"))
                    .header(header::AUTHORIZATION, format!("Bearer {node_token}"))
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(
                        json!({
                            "node_id": node_id,
                            "state": "failed",
                            "error_text": "image pull failed: manifest unknown"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(complete_fail.status(), StatusCode::NO_CONTENT);

        let after_fail = router
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method("GET")
                    .uri(format!("/v1/containers/{container_id}"))
                    .header(header::AUTHORIZATION, format!("Bearer {admin_a}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(after_fail.status(), StatusCode::OK);
        let after_fail_json: serde_json::Value = serde_json::from_slice(
            &axum::body::to_bytes(after_fail.into_body(), 64 * 1024)
                .await
                .unwrap(),
        )
        .unwrap();
        assert_eq!(after_fail_json["container"]["state"], "failed");
        assert_eq!(
            after_fail_json["container"]["last_error"],
            "image pull failed: manifest unknown"
        );
    }
}

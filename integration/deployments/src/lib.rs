use axum::{
    extract::{Path, Request, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use http_body_util::BodyExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;

use deployments::release::ReleaseManager;
use deployments::types::{DeploymentId, DeploymentState, ReleaseId};
use deployments::webhook::{parse_github_webhook, verify_signature, ReplayGuard, WebhookEvent};
use deployments::workflow::{ApprovalRecord, DeploymentRecord};

use fusionpanel_control_store::{ControlStore, OwnershipMode};
use uuid::Uuid;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct DeploymentResourceState {
    pub record: DeploymentRecord,
    pub approval: ApprovalRecord,
}

pub struct AppState {
    pub webhook_secret: Vec<u8>,
    pub replay_guard: Arc<ReplayGuard>,
    pub release_manager: Arc<Mutex<ReleaseManager>>,
    pub control_store: Arc<Mutex<ControlStore>>,
}

pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/webhook", post(webhook_handler))
        .route("/deployments", post(create_deployment))
        .route("/deployments", get(list_deployments))
        .route("/deployments/:id/approve", post(approve_deployment))
        .route("/deployments/:id/rollback", post(rollback_deployment))
        .route("/deployments/:id", get(get_deployment))
        .route(
            "/deployments/:id/health_failure",
            post(health_failure_deployment),
        )
        .with_state(state)
}

#[derive(Serialize)]
struct ErrorResponse {
    error: ErrorDetail,
}

#[derive(Serialize)]
struct ErrorDetail {
    code: String,
    message: String,
}

fn error_response(code: &str, message: &str, status: StatusCode) -> Response {
    let mut r = Response::new(axum::body::Body::from(
        serde_json::to_string(&ErrorResponse {
            error: ErrorDetail {
                code: code.to_string(),
                message: message.to_string(),
            },
        })
        .unwrap(),
    ));
    *r.status_mut() = status;
    r.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    r
}

async fn webhook_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    req: Request<axum::body::Body>,
) -> Response {
    let signature = headers
        .get("X-Hub-Signature-256")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let event_type = headers
        .get("X-GitHub-Event")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    let delivery_id = headers
        .get("X-GitHub-Delivery")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");

    if signature.is_empty() {
        return error_response(
            "unauthorized",
            "missing signature",
            StatusCode::UNAUTHORIZED,
        );
    }

    let (_parts, body) = req.into_parts();

    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return error_response("bad_request", "invalid body", StatusCode::BAD_REQUEST),
    };

    if let Err(_) = verify_signature(&state.webhook_secret, &body_bytes, signature) {
        return error_response(
            "unauthorized",
            "invalid signature",
            StatusCode::UNAUTHORIZED,
        );
    }

    let now = chrono::Utc::now();
    if let Err(_) = state.replay_guard.check_and_record(delivery_id, None, now) {
        return error_response("conflict", "replay detected", StatusCode::CONFLICT);
    }

    match parse_github_webhook(event_type, &body_bytes) {
        Ok(WebhookEvent::Push { .. }) => {
            let mut r = Response::new(axum::body::Body::from("Accepted"));
            *r.status_mut() = StatusCode::ACCEPTED;
            r
        }
        Ok(WebhookEvent::Ping { .. }) => {
            let mut r = Response::new(axum::body::Body::from("Pong"));
            *r.status_mut() = StatusCode::OK;
            r
        }
        Ok(WebhookEvent::Unsupported(_)) => {
            error_response("bad_request", "unsupported event", StatusCode::BAD_REQUEST)
        }
        Err(_) => error_response("bad_request", "invalid payload", StatusCode::BAD_REQUEST),
    }
}

#[derive(Deserialize)]
pub struct CreateDeploymentReq {
    pub initiator: String,
    #[allow(dead_code)]
    pub release_id: String,
}

pub async fn create_deployment(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateDeploymentReq>,
) -> Response {
    let dep_id = DeploymentId::new();
    let mut record = DeploymentRecord::new(dep_id.clone());

    if let Err(e) = record.transition_to(
        DeploymentState::PendingApproval,
        "api",
        "deployment created",
    ) {
        return error_response("conflict", &e.to_string(), StatusCode::CONFLICT);
    }

    let approval = match ApprovalRecord::new(dep_id.clone(), payload.initiator) {
        Ok(a) => a,
        Err(e) => return error_response("bad_request", &e.to_string(), StatusCode::BAD_REQUEST),
    };
    let tenant_id_str = match headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        Some(v) => v,
        None => {
            return error_response(
                "unauthorized",
                "missing x-tenant-id",
                StatusCode::UNAUTHORIZED,
            )
        }
    };
    let tenant_id = match Uuid::parse_str(tenant_id_str) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid x-tenant-id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let state_struct = DeploymentResourceState { record, approval };

    let desired_state = match serde_json::to_value(&state_struct) {
        Ok(v) => v,
        Err(_) => {
            return error_response(
                "internal_error",
                "failed to serialize state",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };

    let resource_id = match Uuid::parse_str(&dep_id.0) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "internal_error",
                "failed to parse deployment id",
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    };

    let mut store = state.control_store.lock().await;
    if let Err(e) = store.create_resource(
        tenant_id,
        resource_id,
        OwnershipMode::TenantOwned,
        desired_state,
    ) {
        return error_response(
            "internal_error",
            &e.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        );
    }
    let mut r = Response::new(axum::body::Body::from(
        serde_json::to_string(&dep_id).unwrap(),
    ));
    *r.status_mut() = StatusCode::CREATED;
    r.headers_mut().insert(
        axum::http::header::CONTENT_TYPE,
        "application/json".parse().unwrap(),
    );
    r
}

#[derive(Deserialize)]
pub struct ApproveDeploymentReq {
    pub approver: String,
    pub release_id: String,
}

pub async fn approve_deployment(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ApproveDeploymentReq>,
) -> Response {
    let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let tenant_id_str = match headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        Some(v) => v,
        None => {
            return error_response(
                "unauthorized",
                "missing x-tenant-id",
                StatusCode::UNAUTHORIZED,
            )
        }
    };
    let tenant_id = match Uuid::parse_str(tenant_id_str) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid x-tenant-id",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let resource_id = match Uuid::parse_str(&dep_id.0) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let mut store = state.control_store.lock().await;
    let resource = match store.get_tenant_resource(tenant_id, resource_id) {
        Ok(r) => r,
        Err(_) => {
            return error_response("not_found", "deployment not found", StatusCode::NOT_FOUND)
        }
    };

    let mut state_struct: DeploymentResourceState =
        match serde_json::from_value(resource.desired_state.clone()) {
            Ok(s) => s,
            Err(_) => {
                return error_response(
                    "internal_error",
                    "failed to parse state",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            }
        };

    if let Err(e) = state_struct.approval.approve(payload.approver) {
        return error_response("unauthorized", &e.to_string(), StatusCode::UNAUTHORIZED);
    }

    let _ =
        state_struct
            .record
            .transition_to(DeploymentState::Building, "api", "deployment approved");
    let _ =
        state_struct
            .record
            .transition_to(DeploymentState::Deploying, "api", "building complete");
    let release_manager = state.release_manager.lock().await;
    let release_id = match ReleaseId::parse(&payload.release_id) {
        Ok(r) => r,
        Err(_) => {
            return error_response("bad_request", "invalid release id", StatusCode::BAD_REQUEST)
        }
    };
    match release_manager.activate_directory_switch(&dep_id, &release_id) {
        Ok(applied) => {
            let _ = state_struct.record.transition_to(
                DeploymentState::Active,
                "api",
                "deployment active",
            );
            let new_state = serde_json::to_value(&state_struct).unwrap();
            if let Err(e) = store.update_tenant_desired_state(
                tenant_id,
                resource_id,
                resource.revision,
                new_state,
            ) {
                return error_response(
                    "internal_error",
                    &e.to_string(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                );
            }
            let mut r = Response::new(axum::body::Body::from(
                serde_json::to_string(&applied).unwrap(),
            ));
            *r.status_mut() = StatusCode::OK;
            r.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                "application/json".parse().unwrap(),
            );
            r
        }
        Err(e) => {
            let _ = state_struct.record.transition_to(
                DeploymentState::Failed,
                "api",
                "deployment failed",
            );
            let new_state = serde_json::to_value(&state_struct).unwrap();
            if let Err(e) = store.update_tenant_desired_state(
                tenant_id,
                resource_id,
                resource.revision,
                new_state,
            ) {
                return error_response(
                    "internal_error",
                    &e.to_string(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                );
            }
            error_response(
                "operation_failed",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            )
        }
    }
}

pub async fn rollback_deployment(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let release_manager = state.release_manager.lock().await;
    let tenant_id_str = match headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        Some(v) => v,
        None => {
            return error_response(
                "unauthorized",
                "missing x-tenant-id",
                StatusCode::UNAUTHORIZED,
            )
        }
    };
    let tenant_id = match Uuid::parse_str(tenant_id_str) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid x-tenant-id",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let resource_id = match Uuid::parse_str(&dep_id.0) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let mut store = state.control_store.lock().await;
    let resource = match store.get_tenant_resource(tenant_id, resource_id) {
        Ok(r) => r,
        Err(_) => {
            return error_response("not_found", "deployment not found", StatusCode::NOT_FOUND)
        }
    };
    let mut state_struct: DeploymentResourceState =
        match serde_json::from_value(resource.desired_state.clone()) {
            Ok(s) => s,
            Err(_) => {
                return error_response(
                    "internal_error",
                    "failed to parse state",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            }
        };
    let prior = match release_manager.current_active_release() {
        Ok(Some(_)) => true,
        _ => false,
    };
    if !prior {
        let _ = state_struct.record.transition_to(
            DeploymentState::RolledBack,
            "api",
            "no active release to rollback",
        );
        let new_state = serde_json::to_value(&state_struct).unwrap();
        if let Err(e) =
            store.update_tenant_desired_state(tenant_id, resource_id, resource.revision, new_state)
        {
            return error_response(
                "internal_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
        let mut r = Response::new(axum::body::Body::from("\"rolled back\""));
        *r.status_mut() = StatusCode::OK;
        r.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        return r;
    }
    match release_manager.rollback_directory_switch(&dep_id, None) {
        Ok(applied) => {
            let _ = state_struct.record.transition_to(
                DeploymentState::RolledBack,
                "api",
                "rolled back successfully",
            );
            let new_state = serde_json::to_value(&state_struct).unwrap();
            if let Err(e) = store.update_tenant_desired_state(
                tenant_id,
                resource_id,
                resource.revision,
                new_state,
            ) {
                return error_response(
                    "internal_error",
                    &e.to_string(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                );
            }
            let mut r = Response::new(axum::body::Body::from(
                serde_json::to_string(&applied).unwrap(),
            ));
            *r.status_mut() = StatusCode::OK;
            r.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                "application/json".parse().unwrap(),
            );
            r
        }
        Err(e) => error_response(
            "operation_failed",
            &e.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

pub async fn health_failure_deployment(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let release_manager = state.release_manager.lock().await;
    let tenant_id_str = match headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        Some(v) => v,
        None => {
            return error_response(
                "unauthorized",
                "missing x-tenant-id",
                StatusCode::UNAUTHORIZED,
            )
        }
    };
    let tenant_id = match Uuid::parse_str(tenant_id_str) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid x-tenant-id",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let resource_id = match Uuid::parse_str(&dep_id.0) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let mut store = state.control_store.lock().await;
    let resource = match store.get_tenant_resource(tenant_id, resource_id) {
        Ok(r) => r,
        Err(_) => {
            return error_response("not_found", "deployment not found", StatusCode::NOT_FOUND)
        }
    };
    let mut state_struct: DeploymentResourceState =
        match serde_json::from_value(resource.desired_state.clone()) {
            Ok(s) => s,
            Err(_) => {
                return error_response(
                    "internal_error",
                    "failed to parse state",
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            }
        };
    let prior = match release_manager.current_active_release() {
        Ok(Some(_)) => true,
        _ => false,
    };
    if prior {
        match release_manager.rollback_directory_switch(&dep_id, None) {
            Ok(applied) => {
                let _ = state_struct.record.transition_to(
                    DeploymentState::Failed,
                    "api",
                    "health check failed, rolled back",
                );
                let new_state = serde_json::to_value(&state_struct).unwrap();
                if let Err(e) = store.update_tenant_desired_state(
                    tenant_id,
                    resource_id,
                    resource.revision,
                    new_state,
                ) {
                    return error_response(
                        "internal_error",
                        &e.to_string(),
                        StatusCode::INTERNAL_SERVER_ERROR,
                    );
                }
                let mut r = Response::new(axum::body::Body::from(
                    serde_json::to_string(&applied).unwrap(),
                ));
                *r.status_mut() = StatusCode::OK;
                r.headers_mut().insert(
                    axum::http::header::CONTENT_TYPE,
                    "application/json".parse().unwrap(),
                );
                return r;
            }
            Err(e) => {
                return error_response(
                    "operation_failed",
                    &e.to_string(),
                    StatusCode::INTERNAL_SERVER_ERROR,
                )
            }
        }
    } else {
        let _ = state_struct.record.transition_to(
            DeploymentState::Failed,
            "api",
            "health check failed, no prior release to rollback to",
        );
        let new_state = serde_json::to_value(&state_struct).unwrap();
        if let Err(e) =
            store.update_tenant_desired_state(tenant_id, resource_id, resource.revision, new_state)
        {
            return error_response(
                "internal_error",
                &e.to_string(),
                StatusCode::INTERNAL_SERVER_ERROR,
            );
        }
        let mut r = Response::new(axum::body::Body::from("\"marked as failed\""));
        *r.status_mut() = StatusCode::OK;
        r.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            "application/json".parse().unwrap(),
        );
        return r;
    }
}

pub async fn get_deployment(
    headers: HeaderMap,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let tenant_id_str = match headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        Some(v) => v,
        None => {
            return error_response(
                "unauthorized",
                "missing x-tenant-id",
                StatusCode::UNAUTHORIZED,
            )
        }
    };
    let tenant_id = match Uuid::parse_str(tenant_id_str) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid x-tenant-id",
                StatusCode::BAD_REQUEST,
            )
        }
    };
    let resource_id = match Uuid::parse_str(&dep_id.0) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid deployment id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let store = state.control_store.lock().await;
    match store.get_tenant_resource(tenant_id, resource_id) {
        Ok(resource) => {
            let state_struct: DeploymentResourceState =
                match serde_json::from_value(resource.desired_state.clone()) {
                    Ok(s) => s,
                    Err(_) => {
                        return error_response(
                            "internal_error",
                            "failed to parse state",
                            StatusCode::INTERNAL_SERVER_ERROR,
                        )
                    }
                };
            let mut r = Response::new(axum::body::Body::from(
                serde_json::to_string(&state_struct.record).unwrap(),
            ));
            *r.status_mut() = StatusCode::OK;
            r.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                "application/json".parse().unwrap(),
            );
            r
        }
        Err(_) => error_response("not_found", "deployment not found", StatusCode::NOT_FOUND),
    }
}

pub async fn list_deployments(headers: HeaderMap, State(state): State<Arc<AppState>>) -> Response {
    let tenant_id_str = match headers.get("x-tenant-id").and_then(|v| v.to_str().ok()) {
        Some(v) => v,
        None => {
            return error_response(
                "unauthorized",
                "missing x-tenant-id",
                StatusCode::UNAUTHORIZED,
            )
        }
    };
    let tenant_id = match Uuid::parse_str(tenant_id_str) {
        Ok(u) => u,
        Err(_) => {
            return error_response(
                "bad_request",
                "invalid x-tenant-id",
                StatusCode::BAD_REQUEST,
            )
        }
    };

    let store = state.control_store.lock().await;
    match store.list_tenant_resources(tenant_id) {
        Ok(resources) => {
            let mut deployments = Vec::new();
            for res in resources {
                if let Ok(state_struct) =
                    serde_json::from_value::<DeploymentResourceState>(res.desired_state)
                {
                    deployments.push(state_struct.record);
                }
            }
            let mut r = Response::new(axum::body::Body::from(
                serde_json::to_string(&deployments).unwrap(),
            ));
            *r.status_mut() = StatusCode::OK;
            r.headers_mut().insert(
                axum::http::header::CONTENT_TYPE,
                "application/json".parse().unwrap(),
            );
            r
        }
        Err(e) => error_response(
            "internal_error",
            &e.to_string(),
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    }
}

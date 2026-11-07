use axum::{
    extract::{State, Request, Path},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use http_body_util::BodyExt;
use std::sync::Arc;
use tokio::sync::Mutex;
use serde::{Deserialize, Serialize};

use deployments::webhook::{verify_signature, ReplayGuard, parse_github_webhook, WebhookEvent};
use deployments::types::{DeploymentId, DeploymentState, ReleaseId};
use deployments::release::ReleaseManager;
use deployments::workflow::{ApprovalRecord, DeploymentRecord};

pub struct AppState {
    pub webhook_secret: Vec<u8>,
    pub replay_guard: Arc<ReplayGuard>,
    pub release_manager: Arc<Mutex<ReleaseManager>>,
    pub deployments: Arc<Mutex<std::collections::HashMap<String, DeploymentRecord>>>,
    pub approvals: Arc<Mutex<std::collections::HashMap<String, ApprovalRecord>>>,
}

pub fn app(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/webhook", post(webhook_handler))
        .route("/deployments", post(create_deployment))
        .route("/deployments/:id/approve", post(approve_deployment))
        .route("/deployments/:id/rollback", post(rollback_deployment))
        .route("/deployments/:id", get(get_deployment))
        .route("/deployments/:id/health_failure", post(health_failure_deployment))
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
        }).unwrap()
    ));
    *r.status_mut() = status;
    r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
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
        return error_response("unauthorized", "missing signature", StatusCode::UNAUTHORIZED);
    }

    let (_parts, body) = req.into_parts();

    let body_bytes = match body.collect().await {
        Ok(collected) => collected.to_bytes(),
        Err(_) => return error_response("bad_request", "invalid body", StatusCode::BAD_REQUEST),
    };

    if let Err(_) = verify_signature(&state.webhook_secret, &body_bytes, signature) {
        return error_response("unauthorized", "invalid signature", StatusCode::UNAUTHORIZED);
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
        },
        Ok(WebhookEvent::Ping { .. }) => {
            let mut r = Response::new(axum::body::Body::from("Pong"));
            *r.status_mut() = StatusCode::OK;
            r
        },
        Ok(WebhookEvent::Unsupported(_)) => error_response("bad_request", "unsupported event", StatusCode::BAD_REQUEST),
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
    State(state): State<Arc<AppState>>,
    Json(payload): Json<CreateDeploymentReq>,
) -> Response {
    let dep_id = DeploymentId::new();
    let mut record = DeploymentRecord::new(dep_id.clone());

    if let Err(e) = record.transition_to(DeploymentState::PendingApproval, "api", "deployment created") {
         return error_response("conflict", &e.to_string(), StatusCode::CONFLICT);
    }

    let approval = match ApprovalRecord::new(dep_id.clone(), payload.initiator) {
        Ok(a) => a,
        Err(e) => return error_response("bad_request", &e.to_string(), StatusCode::BAD_REQUEST),
    };

    let mut deployments = state.deployments.lock().await;
    let mut approvals = state.approvals.lock().await;

    deployments.insert(dep_id.0.clone(), record);
    approvals.insert(dep_id.0.clone(), approval);

    let mut r = Response::new(axum::body::Body::from(serde_json::to_string(&dep_id).unwrap()));
    *r.status_mut() = StatusCode::CREATED;
    r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
    r
}

#[derive(Deserialize)]
pub struct ApproveDeploymentReq {
    pub approver: String,
    pub release_id: String,
}

pub async fn approve_deployment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(payload): Json<ApproveDeploymentReq>,
) -> Response {
    let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => return error_response("bad_request", "invalid deployment id", StatusCode::BAD_REQUEST),
    };

    let mut approvals = state.approvals.lock().await;
    let approval = match approvals.get_mut(&dep_id.0) {
        Some(a) => a,
        None => return error_response("not_found", "deployment not found", StatusCode::NOT_FOUND),
    };

    if let Err(e) = approval.approve(payload.approver) {
        return error_response("unauthorized", &e.to_string(), StatusCode::UNAUTHORIZED);
    }

    let mut deployments = state.deployments.lock().await;
    let record = deployments.get_mut(&dep_id.0).unwrap();
    if let Err(e) = record.transition_to(DeploymentState::Building, "api", "deployment approved") {
        return error_response("conflict", &e.to_string(), StatusCode::CONFLICT);
    }
    let _ = record.transition_to(DeploymentState::Deploying, "api", "building complete");

    let release_manager = state.release_manager.lock().await;
    let release_id = match ReleaseId::parse(&payload.release_id) {
         Ok(r) => r,
         Err(_) => return error_response("bad_request", "invalid release id", StatusCode::BAD_REQUEST),
    };

    match release_manager.activate_directory_switch(&dep_id, &release_id) {
        Ok(applied) => {
            let _ = record.transition_to(DeploymentState::Active, "api", "deployment active");
            let mut r = Response::new(axum::body::Body::from(serde_json::to_string(&applied).unwrap()));
            *r.status_mut() = StatusCode::OK;
            r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
            r
        },
        Err(e) => {
            let _ = record.transition_to(DeploymentState::Failed, "api", "deployment failed");
            error_response("operation_failed", &e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

pub async fn rollback_deployment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
     let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => return error_response("bad_request", "invalid deployment id", StatusCode::BAD_REQUEST),
    };

    let release_manager = state.release_manager.lock().await;

    let mut deployments = state.deployments.lock().await;
    let record = match deployments.get_mut(&dep_id.0) {
        Some(d) => d,
        None => return error_response("not_found", "deployment not found", StatusCode::NOT_FOUND),
    };

    let prior = match release_manager.current_active_release() {
        Ok(Some(_)) => true,
        _ => false,
    };
    if !prior {
        let _ = record.transition_to(DeploymentState::RolledBack, "api", "no active release to rollback");
        let mut r = Response::new(axum::body::Body::from("\"rolled back\""));
        *r.status_mut() = StatusCode::OK;
        r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
        return r;
    }

    match release_manager.rollback_directory_switch(&dep_id, None) {
        Ok(applied) => {
            let _ = record.transition_to(DeploymentState::RolledBack, "api", "rolled back successfully");
            let mut r = Response::new(axum::body::Body::from(serde_json::to_string(&applied).unwrap()));
            *r.status_mut() = StatusCode::OK;
            r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
            r
        },
        Err(e) => error_response("operation_failed", &e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
    }
}

pub async fn health_failure_deployment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
    let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => return error_response("bad_request", "invalid deployment id", StatusCode::BAD_REQUEST),
    };

    let release_manager = state.release_manager.lock().await;

    let mut deployments = state.deployments.lock().await;
    let record = match deployments.get_mut(&dep_id.0) {
        Some(d) => d,
        None => return error_response("not_found", "deployment not found", StatusCode::NOT_FOUND),
    };

    let prior = match release_manager.current_active_release() {
        Ok(Some(_)) => true,
        _ => false,
    };
    if prior {
        match release_manager.rollback_directory_switch(&dep_id, None) {
            Ok(applied) => {
                let _ = record.transition_to(DeploymentState::Failed, "api", "health check failed, rolled back");
                let mut r = Response::new(axum::body::Body::from(serde_json::to_string(&applied).unwrap()));
                *r.status_mut() = StatusCode::OK;
                r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
                return r;
            },
            Err(e) => return error_response("operation_failed", &e.to_string(), StatusCode::INTERNAL_SERVER_ERROR)
        }
    } else {
        let _ = record.transition_to(DeploymentState::Failed, "api", "health check failed, no prior release to rollback to");
        let mut r = Response::new(axum::body::Body::from("\"marked as failed\""));
        *r.status_mut() = StatusCode::OK;
        r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
        return r;
    }
}

pub async fn get_deployment(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Response {
     let dep_id = match DeploymentId::parse(&id) {
        Ok(d) => d,
        Err(_) => return error_response("bad_request", "invalid deployment id", StatusCode::BAD_REQUEST),
    };
    let deployments = state.deployments.lock().await;
    match deployments.get(&dep_id.0) {
        Some(d) => {
            let mut r = Response::new(axum::body::Body::from(serde_json::to_string(&d.clone()).unwrap()));
            *r.status_mut() = StatusCode::OK;
            r.headers_mut().insert(axum::http::header::CONTENT_TYPE, "application/json".parse().unwrap());
            r
        },
        None => error_response("not_found", "deployment not found", StatusCode::NOT_FOUND),
    }
}

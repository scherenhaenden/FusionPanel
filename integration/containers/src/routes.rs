#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use containers::{
    domain::{ContainerId, ContainerSpec, ContainerStatus, ImageRef, MountPoint, PortMapping},
    engine::ContainerEngine,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait AuditLogger: Send + Sync {
    async fn log_event(&self, tenant_id: &str, action: &str, container_id: &str);
}

pub struct AppState {
    pub engine: Arc<dyn ContainerEngine>,
    pub audit_logger: Arc<dyn AuditLogger>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        (StatusCode::INTERNAL_SERVER_ERROR, Json(self)).into_response()
    }
}

pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/v1/containers", post(create_container))
        .route("/v1/containers/{id}", get(inspect_container))
        .route("/v1/containers/{id}", delete(remove_container))
        .route("/v1/containers/{id}/start", post(start_container))
        .route("/v1/containers/{id}/stop", post(stop_container))
        .route("/v1/containers/{id}/restart", post(restart_container))
        .with_state(state)
}

fn get_tenant_id(headers: &HeaderMap) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    headers
        .get("x-tenant-id")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| {
            (
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Missing X-Tenant-ID header".to_string(),
                }),
            )
        })
}

fn validate_tenant_access(
    tenant_id: &str,
    container_id: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let expected_prefix = format!("{}-", tenant_id);
    if !container_id.starts_with(&expected_prefix) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied".to_string(),
            }),
        ));
    }

    let suffix = &container_id[expected_prefix.len()..];
    if uuid::Uuid::parse_str(suffix).is_err() {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid container ID format".to_string(),
            }),
        ));
    }

    Ok(())
}

#[derive(Deserialize)]
pub struct CreateContainerRequest {
    pub image: String,
    pub ports: Option<Vec<PortConfig>>,
    pub mounts: Option<Vec<MountConfig>>,
}

#[derive(Deserialize)]
pub struct PortConfig {
    pub host: u16,
    pub container: u16,
}

#[derive(Deserialize)]
pub struct MountConfig {
    pub source: String,
    pub target: String,
    pub read_only: Option<bool>,
}

#[derive(Serialize)]
pub struct CreateContainerResponse {
    pub id: String,
}

async fn create_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(payload): Json<CreateContainerRequest>,
) -> Result<(StatusCode, Json<CreateContainerResponse>), (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = get_tenant_id(&headers)?;

    let container_id = format!("{}-{}", tenant_id, uuid::Uuid::new_v4());

    let image_ref = ImageRef::new(payload.image).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let cid = ContainerId::new(container_id.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let mut domain_ports = Vec::new();
    if let Some(ports) = payload.ports {
        for port in ports {
            domain_ports.push(PortMapping::new(port.host, port.container).map_err(|e| {
                (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?);
        }
    }

    let mut domain_mounts = Vec::new();
    if let Some(mounts) = payload.mounts {
        for mount in mounts {
            domain_mounts.push(
                MountPoint::new(mount.source, mount.target, mount.read_only.unwrap_or(false))
                    .map_err(|e| {
                        (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: e.to_string(),
                            }),
                        )
                    })?,
            );
        }
    }

    let spec = ContainerSpec::new(cid.clone(), image_ref, domain_ports, domain_mounts);

    state.engine.create(&spec).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state
        .audit_logger
        .log_event(&tenant_id, "create", &container_id)
        .await;

    Ok((
        StatusCode::CREATED,
        Json(CreateContainerResponse { id: container_id }),
    ))
}

async fn start_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = get_tenant_id(&headers)?;
    validate_tenant_access(&tenant_id, &id)?;

    let cid = ContainerId::new(id.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state.engine.start(&cid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state.audit_logger.log_event(&tenant_id, "start", &id).await;

    Ok(StatusCode::NO_CONTENT)
}

async fn stop_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = get_tenant_id(&headers)?;
    validate_tenant_access(&tenant_id, &id)?;

    let cid = ContainerId::new(id.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state.engine.stop(&cid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state.audit_logger.log_event(&tenant_id, "stop", &id).await;

    Ok(StatusCode::NO_CONTENT)
}

async fn restart_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = get_tenant_id(&headers)?;
    validate_tenant_access(&tenant_id, &id)?;

    let cid = ContainerId::new(id.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state.engine.stop(&cid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state.engine.start(&cid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state
        .audit_logger
        .log_event(&tenant_id, "restart", &id)
        .await;

    Ok(StatusCode::NO_CONTENT)
}

async fn remove_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = get_tenant_id(&headers)?;
    validate_tenant_access(&tenant_id, &id)?;

    let cid = ContainerId::new(id.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state.engine.remove(&cid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state
        .audit_logger
        .log_event(&tenant_id, "remove", &id)
        .await;

    Ok(StatusCode::NO_CONTENT)
}

#[derive(Serialize)]
pub struct InspectContainerResponse {
    pub status: String,
}

async fn inspect_container(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<InspectContainerResponse>, (StatusCode, Json<ErrorResponse>)> {
    let tenant_id = get_tenant_id(&headers)?;
    validate_tenant_access(&tenant_id, &id)?;

    let cid = ContainerId::new(id.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let status = state.engine.inspect(&cid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state
        .audit_logger
        .log_event(&tenant_id, "inspect", &id)
        .await;

    let status_str = match status {
        ContainerStatus::Created => "Created",
        ContainerStatus::Running => "Running",
        ContainerStatus::Stopped => "Stopped",
        ContainerStatus::Exited => "Exited",
        ContainerStatus::Removed => "Removed",
    };

    Ok(Json(InspectContainerResponse {
        status: status_str.to_string(),
    }))
}

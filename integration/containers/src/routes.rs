#![forbid(unsafe_code)]

use crate::ports::PortAllocator;
use crate::store::ContainerStore;
use axum::{
    extract::{Extension, Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use containers::{
    domain::{ContainerId, ContainerSpec, ContainerStatus, ImageRef, MountPoint, PortMapping},
    engine::ContainerEngine,
};
use integration_identity::AuthenticatedUser;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[async_trait::async_trait]
pub trait AuditLogger: Send + Sync {
    async fn log_event(&self, tenant_id: &str, action: &str, container_id: &str);
}

pub struct AppState {
    pub engine: Arc<dyn ContainerEngine>,
    pub audit_logger: Arc<dyn AuditLogger>,
    pub port_allocator: Arc<PortAllocator>,
    pub store: Arc<dyn ContainerStore>,
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

pub fn create_router(state: Arc<AppState>) -> Result<Router, String> {
    for port in state
        .store
        .get_all_container_ports()
        .map_err(|e| format!("Failed to load persisted container ports: {e}"))?
    {
        state
            .port_allocator
            .allocate_specific(port)
            .map_err(|e| format!("Failed to restore reserved container port {port}: {e}"))?;
    }

    Ok(Router::new()
        .route("/v1/containers", post(create_container))
        .route("/v1/containers/{id}", get(inspect_container))
        .route("/v1/containers/{id}", delete(remove_container))
        .route("/v1/containers/{id}/start", post(start_container))
        .route("/v1/containers/{id}/stop", post(stop_container))
        .route("/v1/containers/{id}/restart", post(restart_container))
        .with_state(state))
}

fn validate_tenant_access(
    store: &Arc<dyn ContainerStore>,
    tenant_id: &str,
    container_id: &str,
) -> Result<(), (StatusCode, Json<ErrorResponse>)> {
    let expected_prefix = format!("{}-", tenant_id);
    if !container_id.starts_with(&expected_prefix) {
        return Err((
            StatusCode::FORBIDDEN,
            Json(ErrorResponse {
                error: "Access denied: Container does not belong to your tenant ID prefix"
                    .to_string(),
            }),
        ));
    }

    match store.get_container_tenant(container_id) {
        Ok(Some(stored_tenant)) => {
            if stored_tenant != tenant_id {
                return Err((
                    StatusCode::FORBIDDEN,
                    Json(ErrorResponse {
                        error: "Access denied: Tenant mismatch in store".to_string(),
                    }),
                ));
            }
        }
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Container not found in store".to_string(),
                }),
            ));
        }
        Err(_) => {
            return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal error checking container ownership".to_string(),
                }),
            ));
        }
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
    user: Option<Extension<AuthenticatedUser>>,
    Json(payload): Json<CreateContainerRequest>,
) -> Result<(StatusCode, Json<CreateContainerResponse>), (StatusCode, Json<ErrorResponse>)> {
    let user = require_authenticated_user(user)?;
    let tenant_id_str = get_tenant_id_from_user(&user)?;
    let tenant_id = tenant_id_str.as_str();

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
    let mut allocated_ports = Vec::new();
    if let Some(ports) = payload.ports {
        for port in ports {
            let host_port = match state.port_allocator.allocate() {
                Ok(host_port) => host_port,
                Err(e) => {
                    for allocated in &allocated_ports {
                        state.port_allocator.release(*allocated);
                    }
                    return Err((
                        StatusCode::SERVICE_UNAVAILABLE,
                        Json(ErrorResponse {
                            error: e.to_string(),
                        }),
                    ));
                }
            };
            allocated_ports.push(host_port);
            let mapping = match PortMapping::new(host_port, port.container) {
                Ok(mapping) => mapping,
                Err(e) => {
                    for allocated in &allocated_ports {
                        state.port_allocator.release(*allocated);
                    }
                    return Err((
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: e.to_string(),
                        }),
                    ));
                }
            };
            domain_ports.push(mapping);
        }
    }

    let mut domain_mounts = Vec::new();
    if let Some(mounts) = payload.mounts {
        for mount in mounts {
            domain_mounts.push(
                MountPoint::new(mount.source, mount.target, mount.read_only.unwrap_or(false))
                    .map_err(|e| {
                        // Rollback port allocation on error
                        for port in &allocated_ports {
                            state.port_allocator.release(*port);
                        }
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

    let spec = ContainerSpec::new(cid.clone(), image_ref, domain_ports.clone(), domain_mounts);

    // Persist ownership
    if let Err(e) = state
        .store
        .create_container(&container_id, tenant_id, "app", &allocated_ports)
    {
        for port in &allocated_ports {
            state.port_allocator.release(*port);
        }
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to persist ownership: {}", e),
            }),
        ));
    }

    state.engine.create(&spec).await.map_err(|e| {
        // Rollback on engine create failure
        for port in &allocated_ports {
            state.port_allocator.release(*port);
        }
        let _ = state.store.remove_container(&container_id);

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    state
        .audit_logger
        .log_event(tenant_id, "create", &container_id)
        .await;

    Ok((
        StatusCode::CREATED,
        Json(CreateContainerResponse { id: container_id }),
    ))
}

async fn start_container(
    State(state): State<Arc<AppState>>,
    user: Option<Extension<AuthenticatedUser>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let user = require_authenticated_user(user)?;
    let tenant_id = get_tenant_id_from_user(&user)?;
    validate_tenant_access(&state.store, &tenant_id, &id)?;

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
    user: Option<Extension<AuthenticatedUser>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let user = require_authenticated_user(user)?;
    let tenant_id = get_tenant_id_from_user(&user)?;
    validate_tenant_access(&state.store, &tenant_id, &id)?;

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
    user: Option<Extension<AuthenticatedUser>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let user = require_authenticated_user(user)?;
    let tenant_id = get_tenant_id_from_user(&user)?;
    validate_tenant_access(&state.store, &tenant_id, &id)?;

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
    user: Option<Extension<AuthenticatedUser>>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let user = require_authenticated_user(user)?;
    let tenant_id = get_tenant_id_from_user(&user)?;
    validate_tenant_access(&state.store, &tenant_id, &id)?;

    let cid = ContainerId::new(id.clone()).map_err(|e| {
        (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    let allocated_ports = state.store.get_container_ports(&id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to load container port records: {e}"),
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

    // On success, delete ownership
    state.store.remove_container(&id).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Container was removed but ownership cleanup failed: {e}"),
            }),
        )
    })?;
    for port in allocated_ports {
        state.port_allocator.release(port);
    }

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
    user: Option<Extension<AuthenticatedUser>>,
    Path(id): Path<String>,
) -> Result<Json<InspectContainerResponse>, (StatusCode, Json<ErrorResponse>)> {
    let user = require_authenticated_user(user)?;
    let tenant_id = get_tenant_id_from_user(&user)?;
    validate_tenant_access(&state.store, &tenant_id, &id)?;

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

// Helper to extract tenant ID string from bindings
fn require_authenticated_user(
    user: Option<Extension<AuthenticatedUser>>,
) -> Result<AuthenticatedUser, (StatusCode, Json<ErrorResponse>)> {
    user.map(|Extension(user)| user).ok_or_else(|| {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Authentication required".to_string(),
            }),
        )
    })
}

fn get_tenant_id_from_user(
    user: &AuthenticatedUser,
) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
    // Look for the first tenant or site scope and extract the tenant_id
    for binding in &user.bindings {
        match &binding.scope {
            identity::rbac::Scope::Tenant { tenant_id } => {
                return Ok(tenant_id.to_string());
            }
            identity::rbac::Scope::Site { tenant_id, .. } => {
                return Ok(tenant_id.to_string());
            }
            _ => continue,
        }
    }

    // If we only have global scope or no tenant bound, error
    Err((
        StatusCode::FORBIDDEN,
        Json(ErrorResponse {
            error: "User has no tenant scope".to_string(),
        }),
    ))
}

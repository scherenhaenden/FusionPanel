#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use containers::{
    domain::{ContainerId, ContainerSpec, ContainerStatus, ImageRef, MountPoint, PortMapping},
    engine::ContainerEngine,
};
use serde::{Deserialize, Serialize};
use integration_identity::AuthenticatedUser;
use crate::ports::PortAllocator;
use crate::store::ContainerStore;
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
                error: "Access denied: Container does not belong to your tenant ID prefix".to_string(),
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
        },
        Ok(None) => {
            return Err((
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: "Container not found in store".to_string(),
                }),
            ));
        },
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
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<CreateContainerRequest>,
) -> Result<(StatusCode, Json<CreateContainerResponse>), (StatusCode, Json<ErrorResponse>)> {
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
    if let Some(ports) = payload.ports {
        for port in ports {
            let host_port = state.port_allocator.allocate().map_err(|e| {
                 (
                    StatusCode::SERVICE_UNAVAILABLE,
                    Json(ErrorResponse {
                        error: e.to_string(),
                    }),
                )
            })?;
            domain_ports.push(PortMapping::new(host_port, port.container).map_err(|e| {
                state.port_allocator.release(host_port); // Rollback port allocation
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
                        // Rollback port allocation on error
                        for port in &domain_ports {
                            state.port_allocator.release(port.host_port());
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
    state.store.create_container(&container_id, tenant_id, "app").map_err(|e| {
         // Rollback port allocation
         for port in &domain_ports {
             state.port_allocator.release(port.host_port());
         }
         (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to persist ownership: {}", e),
            }),
        )
    })?;

    state.engine.create(&spec).await.map_err(|e| {
        // Rollback on engine create failure
        for port in &domain_ports {
            state.port_allocator.release(port.host_port());
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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
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

    state.engine.remove(&cid).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    // On success, delete ownership
    if let Err(e) = state.store.remove_container(&id) {
         // Log the error but still return ok since container was removed
         eprintln!("Failed to remove container ownership record for {}: {}", id, e);
    }

    // In a full implementation we would inspect the container or store its allocated ports
    // and release them here. For now we will assume the port allocation is managed via store or
    // engine inspect.

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
    Extension(user): Extension<AuthenticatedUser>,
    Path(id): Path<String>,
) -> Result<Json<InspectContainerResponse>, (StatusCode, Json<ErrorResponse>)> {
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
fn get_tenant_id_from_user(user: &AuthenticatedUser) -> Result<String, (StatusCode, Json<ErrorResponse>)> {
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
        })
    ))
}

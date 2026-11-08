#![forbid(unsafe_code)]

pub mod provision;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::{sync::Arc, time::Duration};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone)]
pub struct AppState {
    pub tenant_id: Arc<String>,
    pub jobs: Arc<Mutex<Vec<JobRecord>>>,
    pub instances: Arc<Mutex<Vec<WordPressInstance>>>,
}

#[derive(Clone, Serialize)]
pub struct JobRecord {
    pub id: Uuid,
    pub status: String,
    pub tenant_id: String,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: ErrorDetail,
}

#[derive(Serialize)]
pub struct ErrorDetail {
    pub code: &'static str,
    pub message: String,
}

impl IntoResponse for ErrorResponse {
    fn into_response(self) -> Response {
        let status = match self.error.code {
            "unauthorized" => StatusCode::UNAUTHORIZED,
            "not_found" => StatusCode::NOT_FOUND,
            _ => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(self)).into_response()
    }
}

#[derive(Deserialize)]
pub struct InstallRequest {
    pub domain: String,
    pub title: String,
    pub admin_email: String,
}

#[derive(Serialize)]
pub struct JobResponse {
    pub job_id: Uuid,
    pub status: String,
}

#[derive(Clone, Serialize)]
pub struct WordPressInstance {
    pub id: Uuid,
    pub domain: String,
    pub version: String,
    pub tenant_id: String,
}

#[derive(Deserialize)]
pub struct UpgradeRequest {
    pub version: String,
}

#[derive(Deserialize)]
pub struct PluginsRequest {
    pub plugins: Vec<String>,
}

// Ensure caller is the tenant
fn extract_tenant(headers: &axum::http::HeaderMap) -> Result<String, ErrorResponse> {
    headers
        .get("X-Tenant-ID")
        .and_then(|h| h.to_str().ok())
        .map(|s| s.to_string())
        .ok_or_else(|| ErrorResponse {
            error: ErrorDetail {
                code: "unauthorized",
                message: "Missing or invalid X-Tenant-ID header".to_string(),
            },
        })
}

// Validate ownership against instances state
async fn validate_ownership(
    state: &AppState,
    tenant_id: &str,
    instance_id: Uuid,
) -> Result<WordPressInstance, ErrorResponse> {
    let instances = state.instances.lock().await;

    if let Some(instance) = instances.iter().find(|i| i.id == instance_id) {
        if instance.tenant_id == tenant_id {
            return Ok(instance.clone());
        }
    }

    Err(ErrorResponse {
        error: ErrorDetail {
            code: "not_found", // Return 404 to avoid leaking existence of other tenants' instances
            message: "WordPress instance not found".to_string(),
        },
    })
}

pub async fn install_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Json(req): Json<InstallRequest>,
) -> Result<Json<JobResponse>, ErrorResponse> {
    let tenant_id = extract_tenant(&headers)?;

    if req.domain.is_empty() || req.admin_email.is_empty() {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Invalid domain or email".to_string(),
            },
        });
    }

    let job_id = Uuid::new_v4();

    // Store job as pending
    {
        let mut jobs = state.jobs.lock().await;
        jobs.push(JobRecord {
            id: job_id,
            status: "pending".to_string(),
            tenant_id: tenant_id.clone(),
        });
    }

    // Spawn an async background task to simulate work
    let jobs_ref = state.jobs.clone();
    let instances_ref = state.instances.clone();
    let tenant_id_clone = tenant_id.clone();
    tokio::spawn(async move {
        // Mark job as running
        {
            let mut jobs = jobs_ref.lock().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = "running".to_string();
            }
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Mark job as completed and create the instance
        {
            let mut jobs = jobs_ref.lock().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = "completed".to_string();
            }
        }

        {
            let mut instances = instances_ref.lock().await;
            instances.push(WordPressInstance {
                id: Uuid::new_v4(),
                domain: req.domain,
                version: "6.4.3".to_string(),
                tenant_id: tenant_id_clone,
            });
        }
    });

    Ok(Json(JobResponse {
        job_id,
        status: "pending".to_string(),
    }))
}

pub async fn get_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<WordPressInstance>, ErrorResponse> {
    let tenant_id = extract_tenant(&headers)?;
    let instance = validate_ownership(&state, &tenant_id, id).await?;
    Ok(Json(instance))
}

pub async fn upgrade_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
    Json(req): Json<UpgradeRequest>,
) -> Result<Json<JobResponse>, ErrorResponse> {
    let tenant_id = extract_tenant(&headers)?;

    if req.version.is_empty() {
        return Err(ErrorResponse {
            error: ErrorDetail {
                code: "bad_request",
                message: "Version cannot be empty".to_string(),
            },
        });
    }

    validate_ownership(&state, &tenant_id, id).await?;

    let job_id = Uuid::new_v4();
    {
        let mut jobs = state.jobs.lock().await;
        jobs.push(JobRecord {
            id: job_id,
            status: "pending".to_string(),
            tenant_id: tenant_id.clone(),
        });
    }

    let jobs_ref = state.jobs.clone();
    let instances_ref = state.instances.clone();
    let target_version = req.version;
    tokio::spawn(async move {
        // Mark running
        {
            let mut jobs = jobs_ref.lock().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = "running".to_string();
            }
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        // Upgrade the instance and complete job
        {
            let mut instances = instances_ref.lock().await;
            if let Some(instance) = instances.iter_mut().find(|i| i.id == id) {
                instance.version = target_version;
            }
        }

        {
            let mut jobs = jobs_ref.lock().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = "completed".to_string();
            }
        }
    });

    Ok(Json(JobResponse {
        job_id,
        status: "pending".to_string(),
    }))
}

pub async fn plugins_handler(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
    Path(id): Path<Uuid>,
    Json(_req): Json<PluginsRequest>,
) -> Result<Json<JobResponse>, ErrorResponse> {
    let tenant_id = extract_tenant(&headers)?;

    validate_ownership(&state, &tenant_id, id).await?;

    let job_id = Uuid::new_v4();
    {
        let mut jobs = state.jobs.lock().await;
        jobs.push(JobRecord {
            id: job_id,
            status: "pending".to_string(),
            tenant_id: tenant_id.clone(),
        });
    }

    let jobs_ref = state.jobs.clone();
    tokio::spawn(async move {
        {
            let mut jobs = jobs_ref.lock().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = "running".to_string();
            }
        }

        tokio::time::sleep(Duration::from_millis(50)).await;

        {
            let mut jobs = jobs_ref.lock().await;
            if let Some(job) = jobs.iter_mut().find(|j| j.id == job_id) {
                job.status = "completed".to_string();
            }
        }
    });

    Ok(Json(JobResponse {
        job_id,
        status: "pending".to_string(),
    }))
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/install", post(install_handler))
        .route("/:id", get(get_handler))
        .route("/:id/upgrade", post(upgrade_handler))
        .route("/:id/plugins", post(plugins_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;

    fn test_app() -> (Router, AppState) {
        let app_state = AppState {
            tenant_id: Arc::new("tenant-123".to_string()),
            jobs: Arc::new(Mutex::new(Vec::new())),
            instances: Arc::new(Mutex::new(vec![WordPressInstance {
                id: Uuid::nil(), // We use nil uuid for test fixture
                domain: "example.com".to_string(),
                version: "6.4.3".to_string(),
                tenant_id: "tenant-123".to_string(),
            }])),
        };
        (
            Router::new().nest("/v1/wordpress", router(app_state.clone())),
            app_state,
        )
    }

    #[tokio::test]
    async fn test_install_unauthorized() {
        let (app, _) = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/wordpress/install")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"domain":"test.com","title":"Test","admin_email":"admin@test.com"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_install_authorized_and_job_execution() {
        let (app, state) = test_app();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/wordpress/install")
                    .header("X-Tenant-ID", "tenant-123")
                    .header("Content-Type", "application/json")
                    .body(Body::from(
                        r#"{"domain":"test.com","title":"Test","admin_email":"admin@test.com"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Wait for async task to complete
        tokio::time::sleep(Duration::from_millis(150)).await;

        // Verify job was recorded and marked completed
        let jobs = state.jobs.lock().await;
        assert_eq!(jobs.len(), 1);
        assert_eq!(jobs[0].status, "completed");

        // Verify instance was created
        let instances = state.instances.lock().await;
        assert_eq!(instances.len(), 2); // Initial fixture + new
    }

    #[tokio::test]
    async fn test_get_cross_tenant_rejected() {
        let (app, _) = test_app();
        let id = Uuid::nil();
        let response = app
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("/v1/wordpress/{}", id))
                    .header("X-Tenant-ID", "wrong-tenant") // Wrong tenant
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND); // Returns 404 for cross-tenant per the implementation
    }

    #[tokio::test]
    async fn test_upgrade_cross_tenant_rejected() {
        let (app, _) = test_app();
        let id = Uuid::nil();
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/v1/wordpress/{}/upgrade", id))
                    .header("X-Tenant-ID", "wrong-tenant") // Wrong tenant
                    .header("Content-Type", "application/json")
                    .body(Body::from(r#"{"version":"6.5"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

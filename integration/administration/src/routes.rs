use axum::{
    extract::{State, Path},
    routing::{get, post},
    Json, Router,
    http::{StatusCode, header::AUTHORIZATION},
    response::Response,
    middleware::{self, Next},
    extract::Request,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;
use administration::{
    quotas::{QuotaEngine, ResourceQuotas, QuotaError},
    audit::{AuditLog, AuditEvent, AuditRecord},
    teams::invitation::{Invitation, InvitationError, Role},
};
use tokio::sync::Mutex;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::os::unix::fs::OpenOptionsExt;

#[derive(Clone)]
pub struct AppState {
    pub quotas: Arc<QuotaEngine>,
    pub audit: Arc<Mutex<AuditLog>>,
    pub invitations: Arc<Mutex<DurableInvitations>>,
}

// Very basic durable store using an append-only JSON lines file for invitations, similar to audit
pub struct DurableInvitations {
    invitations: std::collections::HashMap<Uuid, Invitation>,
    path: PathBuf,
}

impl DurableInvitations {
    pub fn new(path: PathBuf) -> Self {
        let mut invitations = std::collections::HashMap::new();
        if path.exists() {
            if let Ok(file) = File::open(&path) {
                for line in BufReader::new(file).lines().filter_map(Result::ok) {
                    if let Ok(inv) = serde_json::from_str::<Invitation>(&line) {
                        invitations.insert(inv.id, inv);
                    }
                }
            }
        }
        Self { invitations, path }
    }

    pub fn get_mut(&mut self, id: &Uuid) -> Option<&mut Invitation> {
        self.invitations.get_mut(id)
    }

    pub fn insert(&mut self, id: Uuid, inv: Invitation) {
        self.invitations.insert(id, inv.clone());
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.path)
        {
            if let Ok(json_line) = serde_json::to_string(&inv) {
                let _ = writeln!(file, "{}", json_line);
            }
        }
    }

    pub fn remove(&mut self, id: &Uuid) -> Option<Invitation> {
        // Soft delete or rewrite whole file in a real impl; for now we just remove from memory.
        // A true robust implementation would rewrite the file.
        self.invitations.remove(id)
    }
}

// Very simple static token auth middleware for demonstration in this PR
async fn require_auth(req: Request, next: Next) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get(AUTHORIZATION);
    if let Some(auth) = auth_header {
        if auth == "Bearer secret-admin-token" {
            return Ok(next.run(req).await);
        }
    }
    Err(StatusCode::UNAUTHORIZED)
}

#[derive(Deserialize)]
pub struct ReserveRequest {
    tenant_id: Uuid,
    resources: ResourceQuotas,
}

#[derive(Deserialize)]
pub struct ReleaseRequest {
    tenant_id: Uuid,
    resources: ResourceQuotas,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    error: String,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        // Auth required routes
        .route("/quotas/reserve", post(reserve_quota))
        .route("/quotas/release", post(release_quota))
        .route("/audit/append", post(append_audit))
        .route("/audit/verify", get(verify_audit))
        .route("/invitations", post(create_invitation))
        .route("/invitations/{id}/accept", post(accept_invitation))
        .route("/invitations/{id}/revoke", post(revoke_invitation))
        .route_layer(middleware::from_fn(require_auth))
        // Public health
        .route("/health", get(|| async { "OK" }))
        .with_state(state)
}

async fn reserve_quota(
    State(state): State<AppState>,
    Json(payload): Json<ReserveRequest>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    match state.quotas.reserve(payload.tenant_id, &payload.resources) {
        Ok(_) => Ok(StatusCode::OK),
        Err(e) => {
            let status = match e {
                QuotaError::LimitExceeded => StatusCode::PAYLOAD_TOO_LARGE,
                QuotaError::LimitsNotConfigured => StatusCode::NOT_FOUND,
            };
            Err((status, Json(ErrorResponse { error: e.to_string() })))
        }
    }
}

async fn release_quota(
    State(state): State<AppState>,
    Json(payload): Json<ReleaseRequest>,
) -> StatusCode {
    state.quotas.release(payload.tenant_id, &payload.resources);
    StatusCode::OK
}

async fn append_audit(
    State(state): State<AppState>,
    Json(event): Json<AuditEvent>,
) -> Json<AuditRecord> {
    let mut audit = state.audit.lock().await;
    let record = audit.append(event);
    Json(record)
}

#[derive(Serialize)]
pub struct VerifyResponse {
    valid: bool,
    error: Option<String>,
}

async fn verify_audit(
    State(state): State<AppState>,
) -> Json<VerifyResponse> {
    let audit = state.audit.lock().await;
    match audit.verify() {
        Ok(_) => Json(VerifyResponse { valid: true, error: None }),
        Err(e) => Json(VerifyResponse { valid: false, error: Some(e.to_string()) }),
    }
}

#[derive(Deserialize)]
pub struct CreateInvitationRequest {
    tenant_id: Uuid,
    role: Role,
    ttl_seconds: u64,
}

async fn create_invitation(
    State(state): State<AppState>,
    Json(payload): Json<CreateInvitationRequest>,
) -> Json<Invitation> {
    let invitation = Invitation::new(payload.tenant_id, payload.role, payload.ttl_seconds);
    let mut invites = state.invitations.lock().await;
    invites.insert(invitation.id, invitation.clone());
    Json(invitation)
}

#[derive(Deserialize)]
pub struct AcceptInvitationRequest {
    token: String,
    current_time: u64,
}

async fn accept_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AcceptInvitationRequest>,
) -> Result<Json<Role>, (StatusCode, Json<ErrorResponse>)> {
    let mut invites = state.invitations.lock().await;
    if let Some(invitation) = invites.get_mut(&id) {
        match invitation.accept(&payload.token, payload.current_time) {
            Ok(role) => {
                // Must persist state change in a real impl
                let inv_clone = invitation.clone();
                invites.insert(id, inv_clone);
                Ok(Json(role))
            },
            Err(e) => {
                let status = match e {
                    InvitationError::InvalidToken => StatusCode::UNAUTHORIZED,
                    InvitationError::Expired => StatusCode::GONE,
                    InvitationError::AlreadyAccepted => StatusCode::CONFLICT,
                    InvitationError::InvalidRole => StatusCode::BAD_REQUEST,
                };
                Err((status, Json(ErrorResponse { error: e.to_string() })))
            }
        }
    } else {
        Err((StatusCode::NOT_FOUND, Json(ErrorResponse { error: "Invitation not found".to_string() })))
    }
}

async fn revoke_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> StatusCode {
    let mut invites = state.invitations.lock().await;
    if invites.remove(&id).is_some() {
        StatusCode::OK
    } else {
        StatusCode::NOT_FOUND
    }
}

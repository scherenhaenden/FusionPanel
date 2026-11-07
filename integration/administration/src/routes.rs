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
use subtle::ConstantTimeEq;

#[derive(Clone)]
pub struct AppState {
    pub quotas: Arc<QuotaEngine>,
    pub audit: Arc<Mutex<AuditLog>>,
    pub invitations: Arc<Mutex<DurableInvitations>>,
    pub admin_token: Arc<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "action", content = "data")]
enum InvitationJournalEntry {
    Upsert(Invitation),
    Revoke { id: Uuid },
}

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
                    if let Ok(entry) = serde_json::from_str::<InvitationJournalEntry>(&line) {
                        match entry {
                            InvitationJournalEntry::Upsert(inv) => {
                                invitations.insert(inv.id, inv);
                            }
                            InvitationJournalEntry::Revoke { id } => {
                                invitations.remove(&id);
                            }
                        }
                    } else if let Ok(inv) = serde_json::from_str::<Invitation>(&line) {
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

    pub fn insert(&mut self, id: Uuid, inv: Invitation) -> Result<(), std::io::Error> {
        let entry = InvitationJournalEntry::Upsert(inv.clone());
        let json_line = serde_json::to_string(&entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.path)?;
        writeln!(file, "{}", json_line)?;
        file.flush()?;
        file.sync_all()?;
        self.invitations.insert(id, inv);
        Ok(())
    }

    pub fn remove(&mut self, id: &Uuid) -> Result<Option<Invitation>, std::io::Error> {
        if !self.invitations.contains_key(id) {
            return Ok(None);
        }
        let entry = InvitationJournalEntry::Revoke { id: *id };
        let json_line = serde_json::to_string(&entry)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .mode(0o600)
            .open(&self.path)?;
        writeln!(file, "{}", json_line)?;
        file.flush()?;
        file.sync_all()?;
        Ok(self.invitations.remove(id))
    }
}

async fn require_auth(
    State(state): State<AppState>,
    req: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = req.headers().get(AUTHORIZATION);
    if let Some(auth) = auth_header.and_then(|h| h.to_str().ok()) {
        if let Some(token) = auth.strip_prefix("Bearer ") {
            let expected_bytes = state.admin_token.as_bytes();
            let given_bytes = token.as_bytes();
            if expected_bytes.ct_eq(given_bytes).into() {
                return Ok(next.run(req).await);
            }
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
        .route_layer(middleware::from_fn_with_state(state.clone(), require_auth))
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
) -> Result<Json<AuditRecord>, (StatusCode, Json<ErrorResponse>)> {
    let mut audit = state.audit.lock().await;
    match audit.append(event) {
        Ok(record) => Ok(Json(record)),
        Err(e) => Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )),
    }
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
) -> Result<Json<Invitation>, (StatusCode, Json<ErrorResponse>)> {
    let invitation = Invitation::new(payload.tenant_id, payload.role, payload.ttl_seconds);
    let mut invites = state.invitations.lock().await;
    if let Err(e) = invites.insert(invitation.id, invitation.clone()) {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to persist invitation: {}", e),
            }),
        ));
    }
    Ok(Json(invitation))
}

#[derive(Deserialize)]
pub struct AcceptInvitationRequest {
    token: String,
    #[serde(default)]
    pub current_time: Option<u64>,
}

async fn accept_invitation(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(payload): Json<AcceptInvitationRequest>,
) -> Result<Json<Role>, (StatusCode, Json<ErrorResponse>)> {
    let server_time = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    let mut invites = state.invitations.lock().await;
    if let Some(invitation) = invites.get_mut(&id) {
        match invitation.accept(&payload.token, server_time) {
            Ok(role) => {
                let inv_clone = invitation.clone();
                if let Err(e) = invites.insert(id, inv_clone) {
                    return Err((
                        StatusCode::INTERNAL_SERVER_ERROR,
                        Json(ErrorResponse {
                            error: format!("Failed to persist accepted invitation: {}", e),
                        }),
                    ));
                }
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
    match invites.remove(&id) {
        Ok(Some(_)) => StatusCode::OK,
        Ok(None) => StatusCode::NOT_FOUND,
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

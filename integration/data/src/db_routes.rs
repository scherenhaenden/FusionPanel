#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State, Extension},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use databases::adapter::{DatabaseAdapter, DbName, DbUser, SecretString, DatabaseError};
use jobs::JobStore;
use integration_identity::AuthenticatedUser;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};

#[derive(Clone)]
pub struct AppState {
    pub adapter: Arc<dyn DatabaseAdapter>,
    pub job_store: Arc<Mutex<JobStore>>,
}

// --- Request/Response Models ---

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDatabaseRequest {
    pub name: String,
    pub charset: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseResponse {
    pub name: String,
    pub charset: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateDatabaseUserRequest {
    pub username: String,
    pub password_hash: String, 
}

#[derive(Debug, Serialize, Deserialize)]
pub struct DatabaseUserResponse {
    pub username: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum Privilege {
    Select,
    Insert,
    Update,
    Delete,
    All,
}

impl std::fmt::Display for Privilege {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Privilege::Select => write!(f, "SELECT"),
            Privilege::Insert => write!(f, "INSERT"),
            Privilege::Update => write!(f, "UPDATE"),
            Privilege::Delete => write!(f, "DELETE"),
            Privilege::All => write!(f, "ALL PRIVILEGES"),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserPrivilegesRequest {
    pub privileges: Vec<Privilege>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
}

// --- Validation Functions ---

pub fn is_valid_identifier(ident: &str) -> bool {
    databases::adapter::validate_identifier(ident).is_ok()
}

pub fn is_valid_charset(charset: &str) -> bool {
    let allowed = ["utf8mb4", "utf8", "latin1", "ascii"];
    allowed.contains(&charset.to_lowercase().as_str())
}

pub fn validate_privileges(privileges: &[Privilege]) -> Result<(), String> {
    if privileges.is_empty() {
        return Err("Privileges list cannot be empty".to_string());
    }
    for p in privileges {
        if databases::adapter::validate_privilege(&p.to_string()).is_err() {
             return Err(format!("Invalid privilege: {:?}", p));
        }
    }
    Ok(())
}

fn map_db_error(err: DatabaseError) -> (StatusCode, Json<ErrorResponse>) {
    let (status, msg) = match err {
        DatabaseError::InvalidInput(m) => (StatusCode::BAD_REQUEST, m),
        DatabaseError::Unauthorized(m) => (StatusCode::FORBIDDEN, m),
        DatabaseError::Conflict(m) => (StatusCode::CONFLICT, m),
        DatabaseError::Unsupported(m) => (StatusCode::NOT_IMPLEMENTED, m),
        DatabaseError::DependencyUnavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
        _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
    };
    (status, Json(ErrorResponse { error: msg }))
}

// --- Handlers ---

pub async fn create_database(
    State(state): State<AppState>,
    Extension(user): Extension<AuthenticatedUser>,
    Json(payload): Json<CreateDatabaseRequest>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&payload.name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }

    let charset = payload.charset.unwrap_or_else(|| "utf8mb4".to_string());
    if !is_valid_charset(&charset) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid charset".to_string(),
            }),
        ));
    }

    let owner = DbUser(user.user_id.to_string()); 
    let db_name = DbName(payload.name.clone());

    let plan = state.adapter.plan_create_database(db_name, owner).await.map_err(map_db_error)?;
    
    // Create Job
    let job_payload = serde_json::to_value(&plan).unwrap();
    let job_id = state.job_store.lock().unwrap().create("create_database", &job_payload).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;
    
    state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Running, None).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

    match state.adapter.apply_create_database(&plan).await {
        Ok(_) => {
            let _ = state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Succeeded, None);
            Ok((
                StatusCode::CREATED,
                Json(DatabaseResponse {
                    name: payload.name,
                    charset,
                }),
            ).into_response())
        }
        Err(e) => {
            let _ = state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Failed, Some(e.to_string()));
            Err(map_db_error(e))
        }
    }
}

pub async fn get_database(
    State(_state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
    Path(db_name): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&db_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }

    Ok((
        StatusCode::OK,
        Json(DatabaseResponse {
            name: db_name,
            charset: "utf8mb4".to_string(), 
        }),
    ).into_response())
}

pub async fn delete_database(
    State(_state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
    Path(db_name): Path<String>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&db_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }

    Err((StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "Database deletion is not supported".into() })))
}

pub async fn create_user(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
    Path(db_name): Path<String>,
    Json(payload): Json<CreateDatabaseUserRequest>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&db_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }
    if !is_valid_identifier(&payload.username) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid username".to_string(),
            }),
        ));
    }

    let username = DbUser(payload.username.clone());
    let password_ref = SecretString(payload.password_hash.clone());

    let plan = state.adapter.plan_create_user(username, password_ref).await.map_err(map_db_error)?;

    let job_payload = serde_json::to_value(&plan).unwrap();
    let job_id = state.job_store.lock().unwrap().create("create_user", &job_payload).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;
    
    state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Running, None).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

    match state.adapter.apply_create_user(&plan).await {
        Ok(_) => {
            let _ = state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Succeeded, None);
            Ok((
                StatusCode::CREATED,
                Json(DatabaseUserResponse {
                    username: payload.username,
                }),
            ).into_response())
        }
        Err(e) => {
             let _ = state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Failed, Some(e.to_string()));
             Err(map_db_error(e))
        }
    }
}

pub async fn get_user(
    State(_state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
    Path((db_name, username)): Path<(String, String)>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&db_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }
    if !is_valid_identifier(&username) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid username".to_string(),
            }),
        ));
    }

    Ok((StatusCode::OK, Json(DatabaseUserResponse { username })).into_response())
}

pub async fn update_user_privileges(
    State(state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
    Path((db_name_str, username_str)): Path<(String, String)>,
    Json(payload): Json<UpdateUserPrivilegesRequest>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&db_name_str) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }
    if !is_valid_identifier(&username_str) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid username".to_string(),
            }),
        ));
    }

    if let Err(e) = validate_privileges(&payload.privileges) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })));
    }

    let db_name = DbName(db_name_str);
    let username = DbUser(username_str);
    let privileges = payload.privileges.iter().map(|p| p.to_string()).collect();

    let plan = state.adapter.plan_grant(db_name, username, privileges).await.map_err(map_db_error)?;
    
    let job_payload = serde_json::to_value(&plan).unwrap();
    let job_id = state.job_store.lock().unwrap().create("update_user_privileges", &job_payload).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;
    
    state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Running, None).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: e.to_string() })))?;

    match state.adapter.apply_grant(&plan).await {
        Ok(_) => {
            let _ = state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Succeeded, None);
            Ok(StatusCode::NO_CONTENT.into_response())
        },
        Err(e) => {
            let _ = state.job_store.lock().unwrap().transition(&job_id, jobs::JobState::Failed, Some(e.to_string()));
            Err(map_db_error(e))
        }
    }
}

pub async fn delete_user(
    State(_state): State<AppState>,
    Extension(_user): Extension<AuthenticatedUser>,
    Path((db_name, username)): Path<(String, String)>,
) -> Result<Response, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&db_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }
    if !is_valid_identifier(&username) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid username".to_string(),
            }),
        ));
    }

    Err((StatusCode::NOT_IMPLEMENTED, Json(ErrorResponse { error: "User deletion is not supported".into() })))
}

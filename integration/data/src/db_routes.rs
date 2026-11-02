#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct AppState {} // Empty state for now, can be extended later

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
    pub password_hash: String, // Assume this is hashed externally or handled securely elsewhere
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

#[derive(Debug, Serialize, Deserialize)]
pub struct UpdateUserPrivilegesRequest {
    pub privileges: Vec<Privilege>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct ErrorResponse {
    pub error: String,
}

// --- Validation Functions ---

/// Validates identifiers (database names, usernames) to prevent SQL injection.
/// Only allows alphanumeric characters and underscores, starting with a letter.
/// Max length: 64 characters.
pub fn is_valid_identifier(ident: &str) -> bool {
    if ident.is_empty() || ident.len() > 64 {
        return false;
    }
    let mut chars = ident.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_alphabetic() {
        return false;
    }
    for c in chars {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return false;
        }
    }
    true
}

/// Validates character sets.
pub fn is_valid_charset(charset: &str) -> bool {
    let allowed = ["utf8mb4", "utf8", "latin1", "ascii"];
    allowed.contains(&charset.to_lowercase().as_str())
}

/// Validates privilege matrix.
pub fn validate_privileges(privileges: &[Privilege]) -> Result<(), String> {
    if privileges.is_empty() {
        return Err("Privileges list cannot be empty".to_string());
    }
    Ok(())
}

// --- Handlers ---

pub async fn create_database(
    State(_state): State<AppState>,
    Json(payload): Json<CreateDatabaseRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
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

    Ok((
        StatusCode::CREATED,
        Json(DatabaseResponse {
            name: payload.name,
            charset,
        }),
    ))
}

pub async fn get_database(
    State(_state): State<AppState>,
    Path(db_name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
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
            charset: "utf8mb4".to_string(), // Mock
        }),
    ))
}

pub async fn delete_database(
    State(_state): State<AppState>,
    Path(db_name): Path<String>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
    if !is_valid_identifier(&db_name) {
        return Err((
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Invalid database name".to_string(),
            }),
        ));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn create_user(
    State(_state): State<AppState>,
    Path(db_name): Path<String>,
    Json(payload): Json<CreateDatabaseUserRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
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

    Ok((
        StatusCode::CREATED,
        Json(DatabaseUserResponse {
            username: payload.username,
        }),
    ))
}

pub async fn get_user(
    State(_state): State<AppState>,
    Path((db_name, username)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
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

    Ok((StatusCode::OK, Json(DatabaseUserResponse { username })))
}

pub async fn update_user_privileges(
    State(_state): State<AppState>,
    Path((db_name, username)): Path<(String, String)>,
    Json(payload): Json<UpdateUserPrivilegesRequest>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
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

    if let Err(e) = validate_privileges(&payload.privileges) {
        return Err((StatusCode::BAD_REQUEST, Json(ErrorResponse { error: e })));
    }

    Ok(StatusCode::NO_CONTENT)
}

pub async fn delete_user(
    State(_state): State<AppState>,
    Path((db_name, username)): Path<(String, String)>,
) -> Result<impl IntoResponse, (StatusCode, Json<ErrorResponse>)> {
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

    Ok(StatusCode::NO_CONTENT)
}

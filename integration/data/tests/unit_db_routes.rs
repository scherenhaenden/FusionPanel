#![forbid(unsafe_code)]

use axum::{
    extract::{Path, State, Extension},
    response::IntoResponse,
    Json,
};
#[path = "../src/db_routes.rs"]
mod db_routes;

use db_routes::{
    create_database, create_user, delete_database, delete_user, get_database, get_user,
    is_valid_charset, is_valid_identifier, update_user_privileges, AppState, CreateDatabaseRequest,
    CreateDatabaseUserRequest, Privilege, UpdateUserPrivilegesRequest,
};

use databases::adapter::{DatabaseAdapter, DatabaseConfig, DatabaseEngine, ConnectionStrategy, SecretString, DbName, DbUser, CreateDbPlan, CreateUserPlan, GrantPlan, DumpPlan, RestorePlan, Result as DbResult};
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use jobs::JobStore;
use integration_identity::AuthenticatedUser;

struct MockAdapter {
    config: DatabaseConfig,
}

#[async_trait]
impl DatabaseAdapter for MockAdapter {
    fn engine(&self) -> DatabaseEngine { self.config.engine.clone() }
    fn config(&self) -> &DatabaseConfig { &self.config }
    async fn check_connection(&self) -> DbResult<()> { Ok(()) }
    async fn plan_create_database(&self, name: DbName, owner: DbUser) -> DbResult<CreateDbPlan> { Ok(CreateDbPlan { db_name: name, owner }) }
    async fn apply_create_database(&self, _plan: &CreateDbPlan) -> DbResult<()> { Ok(()) }
    async fn plan_create_user(&self, username: DbUser, password_ref: SecretString) -> DbResult<CreateUserPlan> { Ok(CreateUserPlan { username, password_ref }) }
    async fn apply_create_user(&self, _plan: &CreateUserPlan) -> DbResult<()> { Ok(()) }
    async fn plan_grant(&self, db_name: DbName, username: DbUser, privileges: Vec<String>) -> DbResult<GrantPlan> { Ok(GrantPlan { db_name, username, privileges }) }
    async fn apply_grant(&self, _plan: &GrantPlan) -> DbResult<()> { Ok(()) }
    async fn plan_dump(&self, db_name: DbName, output_path: String) -> DbResult<DumpPlan> { Ok(DumpPlan { db_name, output_path }) }
    async fn apply_dump(&self, _plan: &DumpPlan) -> DbResult<()> { Ok(()) }
    async fn plan_restore(&self, db_name: DbName, input_path: String) -> DbResult<RestorePlan> { Ok(RestorePlan { db_name, input_path }) }
    async fn apply_restore(&self, _plan: &RestorePlan) -> DbResult<()> { Ok(()) }
}

fn create_mock_state() -> AppState {
    let adapter = Arc::new(MockAdapter {
        config: DatabaseConfig {
            engine: DatabaseEngine::PostgreSQL,
            strategy: ConnectionStrategy::Host { socket_path: None },
            host: "localhost".into(),
            port: 5432,
            admin_user: "admin".into(),
            admin_password_ref: SecretString("dummy".into()),
        },
    });
    let job_store = Arc::new(Mutex::new(JobStore::in_memory().unwrap()));
    AppState { adapter, job_store }
}

fn mock_auth_user() -> AuthenticatedUser {
    AuthenticatedUser {
        user_id: identity::types::UserId::new_v4(),
        bindings: vec![],
    }
}


#[tokio::test]
async fn test_identifier_validation() {
    assert!(is_valid_identifier("my_db"));
    assert!(is_valid_identifier("db123"));
    assert!(is_valid_identifier("DB_test_1"));
    assert!(is_valid_identifier("_db")); // Starts with underscore is allowed by the database module validation rule

    assert!(!is_valid_identifier("")); // empty
    assert!(!is_valid_identifier("1db")); // starts with number
    assert!(!is_valid_identifier("my-db")); // dash not allowed
    assert!(!is_valid_identifier("my db")); // space not allowed
    assert!(!is_valid_identifier("drop table")); // space not allowed
    assert!(!is_valid_identifier("my_db;")); // semicolon not allowed
    assert!(!is_valid_identifier(&"a".repeat(65))); // too long
}

#[tokio::test]
async fn test_charset_validation() {
    assert!(is_valid_charset("utf8mb4"));
    assert!(is_valid_charset("UTF8MB4"));
    assert!(is_valid_charset("utf8"));
    assert!(is_valid_charset("latin1"));
    assert!(is_valid_charset("ascii"));

    assert!(!is_valid_charset(""));
    assert!(!is_valid_charset("utf16"));
    assert!(!is_valid_charset("binary"));
}

#[tokio::test]
async fn test_create_database_invalid_identifier() {
    let state = create_mock_state();
    let req = CreateDatabaseRequest {
        name: "invalid-name".to_string(),
        charset: None,
    };
    let res = create_database(State(state), Extension(mock_auth_user()), Json(req)).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_create_database_invalid_charset() {
    let state = create_mock_state();
    let req = CreateDatabaseRequest {
        name: "valid_name".to_string(),
        charset: Some("invalid_charset".to_string()),
    };
    let res = create_database(State(state), Extension(mock_auth_user()), Json(req)).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_create_database_valid() {
    let state = create_mock_state();
    let req = CreateDatabaseRequest {
        name: "valid_name".to_string(),
        charset: Some("utf8mb4".to_string()),
    };
    let res = create_database(State(state), Extension(mock_auth_user()), Json(req)).await;
    assert!(res.is_ok());
    let r = res.unwrap().into_response();
    assert_eq!(r.status(), axum::http::StatusCode::CREATED);
}

#[tokio::test]
async fn test_get_database_invalid_identifier() {
    let state = create_mock_state();
    let res = get_database(State(state), Extension(mock_auth_user()), Path("invalid-name".to_string())).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_delete_database_invalid_identifier() {
    let state = create_mock_state();
    let res = delete_database(State(state), Extension(mock_auth_user()), Path("invalid-name".to_string())).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_delete_database_unsupported() {
    let state = create_mock_state();
    let res = delete_database(State(state), Extension(mock_auth_user()), Path("valid_db".to_string())).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::NOT_IMPLEMENTED),
        _ => panic!("Expected error"),
    }
}


#[tokio::test]
async fn test_create_user_invalid_identifier() {
    let state = create_mock_state();
    let req = CreateDatabaseUserRequest {
        username: "invalid-user".to_string(),
        password_hash: "hash".to_string(),
    };
    let res = create_user(State(state), Extension(mock_auth_user()), Path("valid_db".to_string()), Json(req)).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_get_user_invalid_identifier() {
    let state = create_mock_state();
    let res = get_user(
        State(state),
        Extension(mock_auth_user()),
        Path(("invalid-db".to_string(), "valid_user".to_string())),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_update_user_privileges_invalid_identifier() {
    let state = create_mock_state();
    let req = UpdateUserPrivilegesRequest {
        privileges: vec![Privilege::Select],
    };
    let res = update_user_privileges(
        State(state),
        Extension(mock_auth_user()),
        Path(("invalid-db".to_string(), "valid_user".to_string())),
        Json(req),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_update_user_privileges_empty_privileges() {
    let state = create_mock_state();
    let req = UpdateUserPrivilegesRequest { privileges: vec![] };
    let res = update_user_privileges(
        State(state),
        Extension(mock_auth_user()),
        Path(("valid_db".to_string(), "valid_user".to_string())),
        Json(req),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_delete_user_invalid_identifier() {
    let state = create_mock_state();
    let res = delete_user(
        State(state),
        Extension(mock_auth_user()),
        Path(("invalid-db".to_string(), "valid_user".to_string())),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_delete_user_unsupported() {
    let state = create_mock_state();
    let res = delete_user(
        State(state),
        Extension(mock_auth_user()),
        Path(("valid_db".to_string(), "valid_user".to_string())),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::NOT_IMPLEMENTED),
        _ => panic!("Expected error"),
    }
}

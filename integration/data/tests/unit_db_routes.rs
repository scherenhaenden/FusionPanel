#![forbid(unsafe_code)]

use axum::extract::{Path, State};
#[path = "../src/db_routes.rs"]
mod db_routes;

use axum::response::IntoResponse;
use axum::Json;
use db_routes::{
    create_database, create_user, delete_database, delete_user, get_database, get_user,
    is_valid_charset, is_valid_identifier, update_user_privileges, AppState, CreateDatabaseRequest,
    CreateDatabaseUserRequest, Privilege, UpdateUserPrivilegesRequest,
};

#[tokio::test]
async fn test_identifier_validation() {
    assert!(is_valid_identifier("my_db"));
    assert!(is_valid_identifier("db123"));
    assert!(is_valid_identifier("DB_test_1"));

    assert!(!is_valid_identifier("")); // empty
    assert!(!is_valid_identifier("1db")); // starts with number
    assert!(!is_valid_identifier("_db")); // starts with underscore
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
    let state = AppState {};
    let req = CreateDatabaseRequest {
        name: "invalid-name".to_string(),
        charset: None,
    };
    let res = create_database(State(state), Json(req)).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_create_database_invalid_charset() {
    let state = AppState {};
    let req = CreateDatabaseRequest {
        name: "valid_name".to_string(),
        charset: Some("invalid_charset".to_string()),
    };
    let res = create_database(State(state), Json(req)).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_create_database_valid() {
    let state = AppState {};
    let req = CreateDatabaseRequest {
        name: "valid_name".to_string(),
        charset: Some("utf8mb4".to_string()),
    };
    let res = create_database(State(state), Json(req)).await;
    assert!(res.is_ok());
    let r = res.unwrap().into_response();
    assert_eq!(r.status(), axum::http::StatusCode::CREATED);
}

#[tokio::test]
async fn test_get_database_invalid_identifier() {
    let state = AppState {};
    let res = get_database(State(state), Path("invalid-name".to_string())).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_delete_database_invalid_identifier() {
    let state = AppState {};
    let res = delete_database(State(state), Path("invalid-name".to_string())).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_create_user_invalid_identifier() {
    let state = AppState {};
    let req = CreateDatabaseUserRequest {
        username: "invalid-user".to_string(),
        password_hash: "hash".to_string(),
    };
    let res = create_user(State(state), Path("valid_db".to_string()), Json(req)).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }

    let state = AppState {};
    let req = CreateDatabaseUserRequest {
        username: "valid_user".to_string(),
        password_hash: "hash".to_string(),
    };
    let res = create_user(State(state), Path("invalid-db".to_string()), Json(req)).await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_get_user_invalid_identifier() {
    let state = AppState {};
    let res = get_user(
        State(state),
        Path(("invalid-db".to_string(), "valid_user".to_string())),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }

    let state = AppState {};
    let res = get_user(
        State(state),
        Path(("valid_db".to_string(), "invalid-user".to_string())),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

#[tokio::test]
async fn test_update_user_privileges_invalid_identifier() {
    let state = AppState {};
    let req = UpdateUserPrivilegesRequest {
        privileges: vec![Privilege::Select],
    };
    let res = update_user_privileges(
        State(state),
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
    let state = AppState {};
    let req = UpdateUserPrivilegesRequest { privileges: vec![] };
    let res = update_user_privileges(
        State(state),
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
    let state = AppState {};
    let res = delete_user(
        State(state),
        Path(("invalid-db".to_string(), "valid_user".to_string())),
    )
    .await;
    match res {
        Err((status, _)) => assert_eq!(status, axum::http::StatusCode::BAD_REQUEST),
        _ => panic!("Expected error"),
    }
}

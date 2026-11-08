#![forbid(unsafe_code)]

#[path = "../src/adapter.rs"]
mod adapter;

#[path = "../src/postgres.rs"]
mod postgres;

use adapter::*;
use postgres::*;
use uuid::Uuid;

#[tokio::test]
async fn test_query_formatting() {
    let pass = SecretString("secret_pass".to_string());
    let config = DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "localhost".to_string(),
        port: 5432,
        admin_user: "postgres".to_string(),
        admin_password_ref: pass.clone(),
    };

    let adapter = PostgresAdapter::new(config);

    // We can just verify validation rules since methods don't return formatted queries directly
    // but plan objects
    let db_name = DbName("test_db".to_string());
    let user_name = DbUser("test_user".to_string());

    let plan = adapter
        .plan_create_database(db_name.clone(), user_name.clone())
        .await
        .unwrap();
    assert_eq!(plan.db_name.0, "test_db");

    // Test that validation prevents injection
    let db_name_invalid = DbName("test_db; DROP DATABASE".to_string());
    let plan_err = adapter
        .plan_create_database(db_name_invalid, user_name.clone())
        .await;
    assert!(plan_err.is_err());

    let db_name_invalid2 = DbName("test_db\"".to_string());
    let plan_err2 = adapter
        .plan_create_database(db_name_invalid2, user_name.clone())
        .await;
    assert!(plan_err2.is_err());

    let privs = vec!["SELECT".to_string(), "DROP DATABASE".to_string()];
    let plan_err3 = adapter
        .plan_grant(db_name.clone(), user_name.clone(), privs)
        .await;
    assert!(plan_err3.is_err());

    let privs_ok = vec!["SELECT".to_string(), "INSERT".to_string()];
    let plan_ok = adapter
        .plan_grant(db_name.clone(), user_name.clone(), privs_ok)
        .await;
    assert!(plan_ok.is_ok());
}

#[tokio::test]
async fn test_database_exists_injection_prevention() {
    let pass = SecretString("secret_pass".to_string());
    let config = DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "localhost".to_string(),
        port: 5432,
        admin_user: "postgres".to_string(),
        admin_password_ref: pass.clone(),
    };

    let adapter = PostgresAdapter::new(config);

    // Should return InvalidInput before attempting to run psql
    let result = adapter
        .database_exists(&DbName("test'; DROP TABLE pg_database; --".to_string()))
        .await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));

    let result = adapter.user_exists(&DbUser("test\"".to_string())).await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));

    let result = adapter
        .drop_database(&DbName("`injection`".to_string()))
        .await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));

    let result = adapter.drop_user(&DbUser("invalid name".to_string())).await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));
}

#[tokio::test]
async fn test_database_identifier_edge_cases() {
    let pass = SecretString("secret_pass".to_string());
    let config = DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "localhost".to_string(),
        port: 5432,
        admin_user: "postgres".to_string(),
        admin_password_ref: pass.clone(),
    };
    let adapter = PostgresAdapter::new(config);

    // Empty identifier
    let res_empty = adapter.database_exists(&DbName("".to_string())).await;
    assert!(matches!(res_empty, Err(DatabaseError::InvalidInput(_))));

    // Oversized identifier (>63 chars)
    let res_large = adapter.database_exists(&DbName("a".repeat(64))).await;
    assert!(matches!(res_large, Err(DatabaseError::InvalidInput(_))));

    // Valid identifiers should not return InvalidInput (they will likely return OperationFailed because there is no DB running)
    let res_valid = adapter
        .database_exists(&DbName("valid_db_1".to_string()))
        .await;
    assert!(!matches!(res_valid, Err(DatabaseError::InvalidInput(_))));
}

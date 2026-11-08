#![forbid(unsafe_code)]

#[path = "../src/adapter.rs"]
mod adapter;

#[path = "../src/mariadb.rs"]
mod mariadb;

use adapter::*;
use mariadb::*;

#[tokio::test]
async fn test_mariadb_injection_prevention() {
    let pass = SecretString("secret_pass".to_string());
    let config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "localhost".to_string(),
        port: 3306,
        admin_user: "root".to_string(),
        admin_password_ref: pass.clone(),
    };

    let adapter = MariaDbAdapter::new(config);

    // Should return InvalidInput before attempting to run mysql
    let result = adapter
        .database_exists(&DbName("test'; DROP DATABASE mysql; --".to_string()))
        .await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));

    let result = adapter.user_exists(&DbUser("test`".to_string())).await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));

    let result = adapter
        .drop_database(&DbName("`injection`".to_string()))
        .await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));

    let result = adapter.drop_user(&DbUser("invalid name".to_string())).await;
    assert!(matches!(result, Err(DatabaseError::InvalidInput(_))));
}

#[tokio::test]
async fn test_mariadb_identifier_edge_cases() {
    let pass = SecretString("secret_pass".to_string());
    let config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "localhost".to_string(),
        port: 3306,
        admin_user: "root".to_string(),
        admin_password_ref: pass.clone(),
    };
    let adapter = MariaDbAdapter::new(config);

    // Empty identifier
    let res_empty = adapter.database_exists(&DbName("".to_string())).await;
    assert!(matches!(res_empty, Err(DatabaseError::InvalidInput(_))));

    // Oversized identifier (>63 chars)
    let res_large = adapter.database_exists(&DbName("a".repeat(64))).await;
    assert!(matches!(res_large, Err(DatabaseError::InvalidInput(_))));

    // Valid identifiers should not return InvalidInput
    let res_valid = adapter
        .database_exists(&DbName("valid_db_1".to_string()))
        .await;
    assert!(!matches!(res_valid, Err(DatabaseError::InvalidInput(_))));
}

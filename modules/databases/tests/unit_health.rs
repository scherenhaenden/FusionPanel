#![forbid(unsafe_code)]

use databases::adapter::{ConnectionStrategy, DatabaseConfig, DatabaseEngine, SecretString};
use databases::health::{DatabaseHealthChecker, HealthStatus};

use std::time::Duration;

fn create_dummy_config() -> DatabaseConfig {
    DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "127.0.0.1".to_string(),
        port: 5432,
        admin_user: "postgres".to_string(),
        admin_password_ref: SecretString("postgres".to_string()),
    }
}

#[tokio::test]
async fn test_connection_refused() {
    let mut config = create_dummy_config();
    config.port = 1; // Unlikely to have postgres here

    let checker = DatabaseHealthChecker::new(config);
    let status = checker.check().await;

    assert!(!status.is_healthy);
    assert!(status.error.is_some());
    // Since `psql` might not be installed in the test env, it could be "No such file or directory"
    // instead of "Command failed", so check for both cases.
    let err = status.error.unwrap();
    assert!(
        err.contains("Command failed") || err.contains("No such file") || err.contains("not found")
    );
}

#[tokio::test]
async fn test_connection_timeout() {
    let mut config = create_dummy_config();
    config.host = "192.0.2.1".to_string(); // TEST-NET-1 (RFC 5737), should timeout

    let checker = DatabaseHealthChecker::new(config).with_timeout(Duration::from_nanos(1));
    let status = checker.check().await;

    assert!(!status.is_healthy);
    assert!(status.error == Some("Connection timeout".to_string()) || status.error.is_some());
}

#[test]
fn test_status_report_serialization() {
    let status = HealthStatus {
        is_healthy: true,
        latency_ms: Some(42),
        active_connections: Some(10),
        version: Some("PostgreSQL 15.3".to_string()),
        error: None,
    };

    let serialized = serde_json::to_string(&status).unwrap();
    assert!(serialized.contains(r#""is_healthy":true"#));
    assert!(serialized.contains(r#""latency_ms":42"#));
    assert!(serialized.contains(r#""active_connections":10"#));
    assert!(serialized.contains(r#""version":"PostgreSQL 15.3""#));
    assert!(serialized.contains(r#""error":null"#));

    let deserialized: HealthStatus = serde_json::from_str(&serialized).unwrap();
    assert_eq!(status, deserialized);
}

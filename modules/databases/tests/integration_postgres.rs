#![forbid(unsafe_code)]

#[path = "../src/adapter.rs"]
mod adapter;

#[path = "../src/postgres.rs"]
mod postgres;

use adapter::*;
use postgres::*;
use std::env;
use std::fs;
use uuid::Uuid;

async fn test_postgres_adapter() -> Result<()> {
    let is_psql_available = tokio::process::Command::new("psql")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_psql_available {
        return Err(DatabaseError::DependencyUnavailable(
            "psql CLI not found".into(),
        ));
    }

    let host = env::var("TEST_PG_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("TEST_PG_PORT")
        .unwrap_or_else(|_| "5432".to_string())
        .parse::<u16>()
        .unwrap_or(5432);
    let user = env::var("TEST_PG_USER").unwrap_or_else(|_| "postgres".to_string());
    let pass = env::var("TEST_PG_PASS").unwrap_or_else(|_| "postgres".to_string());

    let config = DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: host.clone(),
        port,
        admin_user: user.clone(),
        admin_password_ref: SecretString(pass.clone()),
    };

    let adapter = PostgresAdapter::new(config);

    if let Err(e) = adapter.check_connection().await {
        return Err(DatabaseError::DependencyUnavailable(format!(
            "PostgreSQL connection failed: {}",
            e
        )));
    }

    let db_name = DbName(format!("test_db_{}", Uuid::new_v4().simple()));
    let user_name = DbUser(format!("user_{}", Uuid::new_v4().simple()));
    let temp_pass = SecretString("SecurePass123!'".to_string());

    let plan = adapter
        .plan_create_user(user_name.clone(), temp_pass.clone())
        .await?;
    adapter.apply_create_user(&plan).await?;

    let plan = adapter
        .plan_create_database(db_name.clone(), user_name.clone())
        .await?;
    adapter.apply_create_database(&plan).await?;

    let exists = adapter.database_exists(&db_name).await?;
    assert!(exists, "Database should exist");

    // test idempotent drop user
    let res = adapter.drop_user(&user_name).await;
    assert!(res.is_err(), "Cannot drop user before db");

    adapter.drop_database(&db_name).await?;
    let exists = adapter.database_exists(&db_name).await?;
    assert!(!exists, "Database should not exist");

    adapter.drop_user(&user_name).await?;
    let u_exists = adapter.user_exists(&user_name).await?;
    assert!(!u_exists, "User should not exist");

    Ok(())
}

#[tokio::test]
#[ignore = "BLOCKED: requires a disposable PostgreSQL service configured with TEST_PG_*"]
async fn run_integration_tests() {
    let r2 = test_postgres_adapter().await;
    if let Err(DatabaseError::DependencyUnavailable(_msg)) = r2 {
        panic!("RESULT=BLOCKED: PostgreSQL service is unavailable");
    } else {
        r2.unwrap();
    }
}

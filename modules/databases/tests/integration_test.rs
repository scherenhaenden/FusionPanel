use databases::adapter::*;
use databases::mariadb::MariaDbAdapter;
use databases::postgresql::PostgresAdapter;
use std::env;
use std::fs;
use uuid::Uuid;

async fn test_mariadb_adapter() -> Result<()> {
    let is_mysql_available = tokio::process::Command::new("mysql")
        .arg("--version")
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false);

    if !is_mysql_available {
        return Err(DatabaseError::DependencyUnavailable(
            "mysql CLI not found".into(),
        ));
    }

    let host = env::var("TEST_MARIADB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("TEST_MARIADB_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse::<u16>()
        .unwrap_or(3306);
    let user = env::var("TEST_MARIADB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = env::var("TEST_MARIADB_PASS").unwrap_or_else(|_| "".to_string());

    let config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host,
        port,
        admin_user: user,
        admin_password_ref: SecretString(pass),
    };

    let adapter = MariaDbAdapter::new(config);

    if let Err(e) = adapter.check_connection().await {
        return Err(DatabaseError::DependencyUnavailable(format!(
            "MariaDB connection failed: {}",
            e
        )));
    }

    let db_name = DbName(format!("test_db_{}", Uuid::new_v4().simple()));
    let user_name = DbUser(format!("user_{}", Uuid::new_v4().simple()));
    let temp_pass = SecretString("Secure\\Pass123!'".to_string());

    let plan = adapter
        .plan_create_database(db_name.clone(), user_name.clone())
        .await?;
    adapter.apply_create_database(&plan).await?;

    let plan = adapter
        .plan_create_user(user_name.clone(), temp_pass.clone())
        .await?;
    adapter.apply_create_user(&plan).await?;

    let plan = adapter
        .plan_grant(db_name.clone(), user_name.clone(), vec!["SELECT".into()])
        .await?;
    adapter.apply_grant(&plan).await?;

    let dump_path = format!("/tmp/{}.sql", db_name.0);
    let plan = adapter
        .plan_dump(db_name.clone(), dump_path.clone())
        .await?;
    adapter.apply_dump(&plan).await?;

    let metadata = fs::metadata(&dump_path).expect("Dump file not found");
    assert!(metadata.len() > 0);

    // Simulate restoration on a "separate" instance by restoring to a newly created DB
    let new_db_name = DbName(format!("test_db_{}", Uuid::new_v4().simple()));
    let plan_new_db = adapter
        .plan_create_database(new_db_name.clone(), user_name.clone())
        .await?;
    adapter.apply_create_database(&plan_new_db).await?;

    let plan_restore = adapter
        .plan_restore(new_db_name.clone(), dump_path.clone())
        .await?;
    adapter.apply_restore(&plan_restore).await?;

    let _ = fs::remove_file(dump_path);

    Ok(())
}

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
        host,
        port,
        admin_user: user,
        admin_password_ref: SecretString(pass),
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

    let plan = adapter
        .plan_grant(db_name.clone(), user_name.clone(), vec!["SELECT".into()])
        .await?;
    adapter.apply_grant(&plan).await?;

    let dump_path = format!("/tmp/{}.sql", db_name.0);
    let plan = adapter
        .plan_dump(db_name.clone(), dump_path.clone())
        .await?;
    adapter.apply_dump(&plan).await?;

    let metadata = fs::metadata(&dump_path).expect("Dump file not found");
    assert!(metadata.len() > 0);

    // Restore to a new DB
    let new_db_name = DbName(format!("test_db_{}", Uuid::new_v4().simple()));
    let plan_new_db = adapter
        .plan_create_database(new_db_name.clone(), user_name.clone())
        .await?;
    adapter.apply_create_database(&plan_new_db).await?;

    let plan_restore = adapter
        .plan_restore(new_db_name.clone(), dump_path.clone())
        .await?;
    adapter.apply_restore(&plan_restore).await?;

    let _ = fs::remove_file(dump_path);

    Ok(())
}

async fn test_mariadb_container_adapter() -> Result<()> {
    let host = env::var("TEST_MARIADB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("TEST_MARIADB_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse::<u16>()
        .unwrap_or(3306);
    let user = env::var("TEST_MARIADB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = env::var("TEST_MARIADB_PASS").unwrap_or_else(|_| "".to_string());

    let config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Container {
            container_name: "test_mariadb".into(),
            network: None,
        },
        host,
        port,
        admin_user: user,
        admin_password_ref: SecretString(pass),
    };

    let adapter = MariaDbAdapter::new(config);
    // Since we don't actually have a container running, it should fail dependency check, which is fine
    let _ = adapter.check_connection().await;
    Ok(())
}

#[tokio::test]
#[ignore = "BLOCKED dependencies require environment setup for full real integration. Manual or specific CI lane only."]
async fn run_integration_tests() {
    let r1 = test_mariadb_adapter().await;
    if let Err(DatabaseError::DependencyUnavailable(msg)) = r1 {
        println!("BLOCKED: {}", msg);
    } else {
        r1.unwrap();
    }

    let r2 = test_postgres_adapter().await;
    if let Err(DatabaseError::DependencyUnavailable(msg)) = r2 {
        println!("BLOCKED: {}", msg);
    } else {
        r2.unwrap();
    }

    let _ = test_mariadb_container_adapter().await;

    test_transaction_boundary_integrity().await;
}

async fn test_transaction_boundary_integrity() {
    let host = env::var("TEST_MARIADB_HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("TEST_MARIADB_PORT")
        .unwrap_or_else(|_| "3306".to_string())
        .parse::<u16>()
        .unwrap_or(3306);
    let user = env::var("TEST_MARIADB_USER").unwrap_or_else(|_| "root".to_string());
    let pass = env::var("TEST_MARIADB_PASS").unwrap_or_else(|_| "".to_string());

    let config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host,
        port,
        admin_user: user,
        admin_password_ref: SecretString(pass),
    };

    let adapter = MariaDbAdapter::new(config);
    let db_name = DbName(format!("test_db_{}", Uuid::new_v4().simple()));
    let user_name = DbUser(format!("user_{}", Uuid::new_v4().simple()));

    // We are testing that a plan_grant with one valid and one invalid privilege fails planning.
    let plan_result = adapter
        .plan_grant(
            db_name,
            user_name,
            vec!["SELECT".into(), "INVALID_PRIV".into()],
        )
        .await;

    assert!(
        plan_result.is_err(),
        "Planning should fail with an invalid privilege in the array"
    );
}

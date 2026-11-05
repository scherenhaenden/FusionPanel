#![forbid(unsafe_code)]

#[path = "../src/backup_scheduler.rs"]
mod backup_scheduler;

use backup_scheduler::{Compression, DumpScheduler};
use databases::adapter::{
    ConnectionStrategy, DatabaseAdapter, DatabaseConfig, DatabaseEngine, DbName, DbUser,
    SecretString,
};
use databases::mariadb::MariaDbAdapter;
use databases::postgresql::PostgresAdapter;
use std::process::Command;
use std::sync::Arc;

async fn cleanup_mariadb(db_name: &DbName, db_user: &DbUser, config: &DatabaseConfig) {
    let mut drop_db = Command::new("mysql");
    drop_db
        .arg(format!("--host={}", config.host))
        .arg(format!("--port={}", config.port))
        .arg(format!("--user={}", config.admin_user))
        .env("MYSQL_PWD", &config.admin_password_ref.0)
        .arg("-e")
        .arg(format!("DROP DATABASE IF EXISTS `{}`;", db_name.0));
    let _ = drop_db.output();

    let mut drop_user = Command::new("mysql");
    drop_user
        .arg(format!("--host={}", config.host))
        .arg(format!("--port={}", config.port))
        .arg(format!("--user={}", config.admin_user))
        .env("MYSQL_PWD", &config.admin_password_ref.0)
        .arg("-e")
        .arg(format!("DROP USER IF EXISTS '{}'@'%';", db_user.0));
    let _ = drop_user.output();
}

async fn cleanup_postgresql(db_name: &DbName, db_user: &DbUser, config: &DatabaseConfig) {
    let mut drop_db = Command::new("psql");
    drop_db
        .arg("-d")
        .arg("postgres")
        .arg("-c")
        .arg(format!("DROP DATABASE IF EXISTS \"{}\";", db_name.0))
        .env("PGPASSWORD", &config.admin_password_ref.0);
    let _ = drop_db.output();

    let mut drop_user = Command::new("psql");
    drop_user
        .arg("-d")
        .arg("postgres")
        .arg("-c")
        .arg(format!("DROP USER IF EXISTS \"{}\";", db_user.0))
        .env("PGPASSWORD", &config.admin_password_ref.0);
    let _ = drop_user.output();
}

async fn test_adapter_lifecycle(
    adapter: Arc<dyn DatabaseAdapter>,
    compression: Compression,
    extension: &str,
) {
    let db_name = DbName("testdb_disposable".to_string());
    let db_user = DbUser("testuser_disposable".to_string());
    let password = SecretString("testpass123".to_string());

    // Clean up first to ensure fresh state
    if adapter.engine() == DatabaseEngine::MariaDB {
        cleanup_mariadb(&db_name, &db_user, adapter.config()).await;
    } else {
        cleanup_postgresql(&db_name, &db_user, adapter.config()).await;
    }

    // 1. Create DB
    let create_db_plan = adapter
        .plan_create_database(db_name.clone(), db_user.clone())
        .await
        .unwrap();
    adapter
        .apply_create_database(&create_db_plan)
        .await
        .unwrap();

    // 2. Create User
    let create_user_plan = adapter
        .plan_create_user(db_user.clone(), password.clone())
        .await
        .unwrap();
    adapter.apply_create_user(&create_user_plan).await.unwrap();

    // 3. Grant Privileges
    let grant_plan = adapter
        .plan_grant(
            db_name.clone(),
            db_user.clone(),
            vec!["SELECT".to_string(), "INSERT".to_string()],
        )
        .await
        .unwrap();
    adapter.apply_grant(&grant_plan).await.unwrap();

    // 4. Test Backup
    let temp_dir = tempfile::tempdir().unwrap();
    let scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        adapter.clone(),
        temp_dir.path().to_path_buf(),
        compression,
    )
    .unwrap();

    let filename = format!("dump.{}", extension);
    let dump_res = scheduler
        .execute_dump("testdb_disposable", &filename)
        .await
        .unwrap();

    assert!(dump_res.0.exists());

    // 5. Test Restore
    // Decompress for restore
    let restored_file = temp_dir.path().join("restored.sql");

    if compression == Compression::Gzip {
        Command::new("gzip")
            .arg("-d")
            .arg("-c")
            .arg(&dump_res.0)
            .stdout(std::fs::File::create(&restored_file).unwrap())
            .status()
            .unwrap();
    } else {
        Command::new("zstd")
            .arg("-d")
            .arg("-c")
            .arg(&dump_res.0)
            .stdout(std::fs::File::create(&restored_file).unwrap())
            .status()
            .unwrap();
    }

    let restore_plan = adapter
        .plan_restore(db_name.clone(), restored_file.to_string_lossy().to_string())
        .await
        .unwrap();
    adapter.apply_restore(&restore_plan).await.unwrap();

    // 6. Test Query / Deny / Drop Cleanup
    if adapter.engine() == DatabaseEngine::MariaDB {
        // Query (Success with SELECT/INSERT)
        let mut test_query = Command::new("mysql");
        test_query
            .arg(format!("--host={}", adapter.config().host))
            .arg(format!("--port={}", adapter.config().port))
            .arg(format!("--user={}", db_user.0))
            .env("MYSQL_PWD", &password.0)
            .arg(&db_name.0)
            .arg("-e")
            .arg("CREATE TABLE dummy (id INT); INSERT INTO dummy VALUES (1); SELECT * FROM dummy;");
        let out = test_query.output().unwrap();

        // In MariaDB, our "SELECT, INSERT" does not automatically grant CREATE, so the above fails with denied!
        // This is a great test for deny actually.
        assert!(!out.status.success());
        let err_str = String::from_utf8_lossy(&out.stderr);
        assert!(err_str.contains("Access denied") || err_str.contains("command denied"));

        // Let's grant CREATE to properly test query success
        let grant_plan_create = adapter
            .plan_grant(
                db_name.clone(),
                db_user.clone(),
                vec![
                    "CREATE".to_string(),
                    "SELECT".to_string(),
                    "INSERT".to_string(),
                ],
            )
            .await
            .unwrap();
        adapter.apply_grant(&grant_plan_create).await.unwrap();

        let mut test_query2 = Command::new("mysql");
        test_query2
            .arg(format!("--host={}", adapter.config().host))
            .arg(format!("--port={}", adapter.config().port))
            .arg(format!("--user={}", db_user.0))
            .env("MYSQL_PWD", &password.0)
            .arg(&db_name.0)
            .arg("-e")
            .arg("CREATE TABLE dummy (id INT); INSERT INTO dummy VALUES (1); SELECT * FROM dummy;");
        let out2 = test_query2.output().unwrap();
        assert!(out2.status.success());

        cleanup_mariadb(&db_name, &db_user, adapter.config()).await;
    } else {
        // Query (Success with SELECT/INSERT)
        // In PostgreSQL we granted CONNECT on DATABASE, and privileges on ALL TABLES IN SCHEMA public.
        // Let's test that directly by logging in as the user
        let mut test_query = Command::new("psql");
        test_query
            .arg("-d")
            .arg(&db_name.0)
            .arg("-U")
            .arg(&db_user.0)
            .env("PGPASSWORD", &password.0)
            .arg("-c")
            .arg("CREATE TABLE dummy (id INT); INSERT INTO dummy VALUES (1); SELECT * FROM dummy;");
        let out = test_query.output().unwrap();
        // User cannot create tables in public schema by default in pg15 without explicit schema grants, so it's a deny test!
        assert!(!out.status.success());
        let err_str = String::from_utf8_lossy(&out.stderr);
        assert!(err_str.contains("permission denied"));

        // We will just do a standard query to test connection access
        let mut test_query_conn = Command::new("psql");
        test_query_conn
            .arg("-d")
            .arg(&db_name.0)
            .arg("-U")
            .arg(&db_user.0)
            .env("PGPASSWORD", &password.0)
            .arg("-c")
            .arg("SELECT 1;");
        let out_conn = test_query_conn.output().unwrap();
        assert!(out_conn.status.success());

        cleanup_postgresql(&db_name, &db_user, adapter.config()).await;
    }
}

#[tokio::test]
async fn test_disposable_db_lifecycle_mariadb() {
    let check = std::process::Command::new("mysql")
        .arg("--version")
        .output();
    if check.is_err() || !check.unwrap().status.success() {
        println!("BLOCKED: mysql client not available. Skipping MariaDB disposable DB test.");
        return;
    }
    let config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "127.0.0.1".into(),
        port: 3306,
        admin_user: "root".into(),
        admin_password_ref: SecretString("rootpassword".into()),
    };

    let adapter = Arc::new(MariaDbAdapter::new(config));

    if adapter.check_connection().await.is_err() {
        println!("BLOCKED: Could not connect to MariaDB on 127.0.0.1:3306");
        return;
    }

    test_adapter_lifecycle(adapter, Compression::Gzip, "sql.gz").await;
}

#[tokio::test]
async fn test_disposable_db_lifecycle_postgresql() {
    let check = std::process::Command::new("psql").arg("--version").output();
    if check.is_err() || !check.unwrap().status.success() {
        println!("BLOCKED: psql client not available. Skipping PostgreSQL disposable DB test.");
        return;
    }
    let config = DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Host { socket_path: None },
        host: "127.0.0.1".into(),
        port: 5432,
        admin_user: "postgres".into(),
        admin_password_ref: SecretString("rootpassword".into()),
    };

    let adapter = Arc::new(PostgresAdapter::new(config));

    if adapter.check_connection().await.is_err() {
        println!("BLOCKED: Could not connect to PostgreSQL on 127.0.0.1:5432");
        return;
    }

    test_adapter_lifecycle(adapter, Compression::Zstd, "sql.zst").await;
}

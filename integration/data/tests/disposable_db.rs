#![forbid(unsafe_code)]

use databases::adapter::{DatabaseConfig, DatabaseEngine, ConnectionStrategy, SecretString, DbName, DbUser, DatabaseAdapter};
use databases::mariadb::MariaDbAdapter;
use databases::postgresql::PostgresAdapter;
use integration_data::backup_scheduler::{Compression, DumpScheduler};
use std::sync::Arc;
use std::process::Command;

async fn test_adapter_lifecycle(adapter: Arc<dyn DatabaseAdapter>, compression: Compression, extension: &str) {
    let db_name = DbName("testdb_disposable".to_string());
    let db_user = DbUser("testuser_disposable".to_string());
    let password = SecretString("testpass123".to_string());

    // 1. Create DB
    let create_db_plan = adapter.plan_create_database(db_name.clone(), db_user.clone()).await.unwrap();
    adapter.apply_create_database(&create_db_plan).await.unwrap();

    // 2. Create User
    let create_user_plan = adapter.plan_create_user(db_user.clone(), password).await.unwrap();
    adapter.apply_create_user(&create_user_plan).await.unwrap();

    // 3. Grant Privileges
    let grant_plan = adapter.plan_grant(db_name.clone(), db_user.clone(), vec!["SELECT".to_string(), "INSERT".to_string()]).await.unwrap();
    adapter.apply_grant(&grant_plan).await.unwrap();

    // 4. Test Backup
    let temp_dir = tempfile::tempdir().unwrap();
    let scheduler = DumpScheduler::new("0 0 0 * * * *", adapter.clone(), temp_dir.path().to_path_buf(), compression).unwrap();
    
    let filename = format!("dump.{}", extension);
    let dump_res = scheduler.execute_dump("testdb_disposable", &filename).await.unwrap();
    
    assert!(dump_res.0.exists());

    // 5. Test Restore
    // Decompress for restore 
    let restored_file = temp_dir.path().join("restored.sql");
    
    if compression == Compression::Gzip {
        Command::new("gzip").arg("-d").arg("-c").arg(&dump_res.0).stdout(std::fs::File::create(&restored_file).unwrap()).status().unwrap();
    } else {
        Command::new("zstd").arg("-d").arg("-c").arg(&dump_res.0).stdout(std::fs::File::create(&restored_file).unwrap()).status().unwrap();
    }

    let restore_plan = adapter.plan_restore(db_name.clone(), restored_file.to_string_lossy().to_string()).await.unwrap();
    adapter.apply_restore(&restore_plan).await.unwrap();
}

#[tokio::test]
async fn test_disposable_db_lifecycle_mariadb() {
    let check = std::process::Command::new("mysql").arg("--version").output();
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
        admin_password_ref: SecretString("rootpassword".into()), // Needs to match local dev setup
    };
    
    let adapter = Arc::new(MariaDbAdapter::new(config));
    
    // Check if we can connect to a real instance, otherwise BLOCK
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
    
    // Check connection
    if adapter.check_connection().await.is_err() {
        println!("BLOCKED: Could not connect to PostgreSQL on 127.0.0.1:5432");
        return;
    }

    test_adapter_lifecycle(adapter, Compression::Zstd, "sql.zst").await;
}

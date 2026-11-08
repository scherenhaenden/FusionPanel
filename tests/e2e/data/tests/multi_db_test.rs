#![forbid(unsafe_code)]

use std::process::Command;
use std::time::Duration;

#[tokio::test]
async fn test_mariadb_and_postgres_dumps() {
    let docker_check = Command::new("docker").arg("info").output();
    if docker_check.is_err() || !docker_check.unwrap().status.success() {
        panic!("BLOCKED: Docker is not available or not running.");
    }

    let compose_check = Command::new("docker").args(["compose", "version"]).output();
    if compose_check.is_err() || !compose_check.unwrap().status.success() {
        panic!("BLOCKED: docker compose is not available.");
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".into());
    let compose_file = format!("{}/tests/docker-compose.yml", manifest_dir);

    // Bring down any existing containers
    let _ = Command::new("docker")
        .args(["compose", "-f", &compose_file, "down", "-v"])
        .output();

    // Start containers
    let up_output = Command::new("docker")
        .args(["compose", "-f", &compose_file, "up", "-d"])
        .output()
        .expect("Failed to execute docker compose up");

    if !up_output.status.success() {
        let err = String::from_utf8_lossy(&up_output.stderr);
        panic!(
            "BLOCKED: Failed to start docker compose (restricted environment): {}",
            err
        );
    }

    // Wait for databases to be ready
    let mut mariadb_ready = false;
    let mut postgres_ready = false;

    // Give it up to 60 seconds
    for _ in 0..12 {
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Try executing a query in mariadb
        let mariadb_query = Command::new("docker")
            .args([
                "compose",
                "-f",
                &compose_file,
                "exec",
                "-T",
                "mariadb",
                "mysql",
                "-utestuser",
                "-ptestpassword",
                "-e",
                "SELECT 1;",
            ])
            .output();

        if let Ok(output) = mariadb_query {
            if output.status.success() {
                mariadb_ready = true;
            }
        }

        // Try executing a query in postgres
        let pg_query = Command::new("docker")
            .args([
                "compose",
                "-f",
                &compose_file,
                "exec",
                "-T",
                "postgres",
                "psql",
                "-U",
                "testuser",
                "-d",
                "testdb",
                "-c",
                "SELECT 1;",
            ])
            .output();

        if let Ok(output) = pg_query {
            if output.status.success() {
                postgres_ready = true;
            }
        }

        if mariadb_ready && postgres_ready {
            break;
        }
    }

    if !mariadb_ready || !postgres_ready {
        // Teardown
        let _ = Command::new("docker")
            .args(["compose", "-f", &compose_file, "down", "-v"])
            .output();
        panic!("BLOCKED: Databases did not become ready in time.");
    }

    use databases::adapter::{
        ConnectionStrategy, DatabaseAdapter, DatabaseConfig, DatabaseEngine, DbName, DbUser,
        SecretString,
    };
    use databases::mariadb::MariaDbAdapter;
    use databases::postgresql::PostgresAdapter;
    use integration_data::backup_scheduler::{BackupReceipt, StorageBackend};
    use integration_data::backup_scheduler::{Compression, DumpScheduler};
    use std::sync::Arc;

    struct RealFileStorageBackend {
        pub destination_dir: std::path::PathBuf,
    }

    #[async_trait::async_trait]
    impl StorageBackend for RealFileStorageBackend {
        async fn upload(
            &self,
            local_path: &std::path::Path,
            remote_filename: &str,
        ) -> Result<BackupReceipt, String> {
            let dest_path = self.destination_dir.join(remote_filename);
            std::fs::copy(local_path, &dest_path).map_err(|e| e.to_string())?;

            let metadata = std::fs::metadata(&dest_path).map_err(|e| e.to_string())?;

            use sha2::{Digest, Sha256};
            let mut file = std::fs::File::open(&dest_path).map_err(|e| e.to_string())?;
            let mut hasher = Sha256::new();
            std::io::copy(&mut file, &mut std::io::Sink::default()).map_err(|e| e.to_string())?; // Not used this way actually
                                                                                                 // Correct way for Sha256 reading
            let mut buffer = Vec::new();
            use std::io::Read;
            file.read_to_end(&mut buffer).map_err(|e| e.to_string())?;
            hasher.update(&buffer);
            let result = hasher.finalize();
            let hash_sha256 = hex::encode(result);

            Ok(BackupReceipt {
                path_or_key: dest_path.to_string_lossy().to_string(),
                hash_sha256,
                size_bytes: metadata.len(),
            })
        }
        async fn delete(&self, remote_filename: &str) -> Result<(), String> {
            let dest_path = self.destination_dir.join(remote_filename);
            if dest_path.exists() {
                std::fs::remove_file(dest_path).map_err(|e| e.to_string())?;
            }
            Ok(())
        }
    }

    let mariadb_config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Container {
            container_name: "mariadb".to_string(),
            network: None,
        },
        host: "127.0.0.1".into(),
        port: 3306,
        admin_user: "root".into(), // test database admin should be root or powerful enough
        admin_password_ref: SecretString("rootpassword".into()),
    };
    let mariadb_adapter = Arc::new(MariaDbAdapter::new(mariadb_config));

    let pg_config = DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Container {
            container_name: "postgres".to_string(),
            network: None,
        },
        host: "127.0.0.1".into(),
        port: 5432,
        admin_user: "testuser".into(), // testuser is superuser in compose
        admin_password_ref: SecretString("rootpassword".into()),
    };
    let pg_adapter = Arc::new(PostgresAdapter::new(pg_config));

    // Ensure adapters can connect
    if let Err(e) = mariadb_adapter.check_connection().await {
        panic!("BLOCKED: MariaDB container connection failed: {:?}", e);
    }
    if let Err(e) = pg_adapter.check_connection().await {
        panic!("BLOCKED: PostgreSQL container connection failed: {:?}", e);
    }

    // Database Lifecycle testing
    // MariaDB: Plan and Apply create user, database, grant
    let m_user_plan1 = mariadb_adapter
        .plan_create_user(DbUser("m_user1".into()), SecretString("m_pass1".into()))
        .await
        .unwrap();
    mariadb_adapter
        .apply_create_user(&m_user_plan1)
        .await
        .unwrap();

    let m_user_plan2 = mariadb_adapter
        .plan_create_user(DbUser("m_user2".into()), SecretString("m_pass2".into()))
        .await
        .unwrap();
    mariadb_adapter
        .apply_create_user(&m_user_plan2)
        .await
        .unwrap();

    let m_db_plan1 = mariadb_adapter
        .plan_create_database(DbName("m_db1".into()), DbUser("m_user1".into()))
        .await
        .unwrap();
    mariadb_adapter
        .apply_create_database(&m_db_plan1)
        .await
        .unwrap();

    let m_grant_plan1 = mariadb_adapter
        .plan_grant(DbName("m_db1".into()), DbUser("m_user1".into()), vec![])
        .await
        .unwrap();
    mariadb_adapter.apply_grant(&m_grant_plan1).await.unwrap();

    // MariaDB: Create table and insert data (positive auth test as the newly created user)
    let create_mariadb = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "mariadb", "mysql", "-um_user1", "-pm_pass1", "m_db1", "-e",
              "CREATE TABLE my_table (id INT PRIMARY KEY, data VARCHAR(255)); INSERT INTO my_table (id, data) VALUES (1, 'mariadb_test_data');"])
        .output()
        .expect("Failed to create table in mariadb");
    assert!(
        create_mariadb.status.success(),
        "MariaDB positive access and table creation failed: {:?}",
        String::from_utf8_lossy(&create_mariadb.stderr)
    );

    // MariaDB isolation check
    let isolate_mariadb = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_file,
            "exec",
            "-T",
            "mariadb",
            "mysql",
            "-um_user2",
            "-pm_pass2",
            "m_db1",
            "-e",
            "SELECT 1;",
        ])
        .output()
        .unwrap();
    assert!(
        !isolate_mariadb.status.success(),
        "MariaDB isolation failed: m_user2 could access m_db1"
    );

    // Postgres: Plan and Apply create user, database, grant
    let pg_user_plan1 = pg_adapter
        .plan_create_user(DbUser("p_user1".into()), SecretString("p_pass1".into()))
        .await
        .unwrap();
    pg_adapter.apply_create_user(&pg_user_plan1).await.unwrap();

    let pg_user_plan2 = pg_adapter
        .plan_create_user(DbUser("p_user2".into()), SecretString("p_pass2".into()))
        .await
        .unwrap();
    pg_adapter.apply_create_user(&pg_user_plan2).await.unwrap();

    let pg_db_plan1 = pg_adapter
        .plan_create_database(DbName("p_db1".into()), DbUser("p_user1".into()))
        .await
        .unwrap();
    pg_adapter
        .apply_create_database(&pg_db_plan1)
        .await
        .unwrap();

    let pg_grant_plan1 = pg_adapter
        .plan_grant(DbName("p_db1".into()), DbUser("p_user1".into()), vec![])
        .await
        .unwrap();
    pg_adapter.apply_grant(&pg_grant_plan1).await.unwrap();

    // Postgres: Create table and insert data (positive auth test as the newly created user)
    let create_pg = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "-e", "PGPASSWORD=p_pass1", "postgres", "psql", "-U", "p_user1", "-d", "p_db1", "-c",
              "CREATE TABLE my_table (id INT PRIMARY KEY, data VARCHAR(255)); INSERT INTO my_table (id, data) VALUES (1, 'postgres_test_data');"])
        .output()
        .expect("Failed to create table in postgres");
    assert!(
        create_pg.status.success(),
        "Postgres positive access and table creation failed: {:?}",
        String::from_utf8_lossy(&create_pg.stderr)
    );

    // Postgres isolation check
    let isolate_pg = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_file,
            "exec",
            "-T",
            "-e",
            "PGPASSWORD=p_pass2",
            "postgres",
            "psql",
            "-U",
            "p_user2",
            "-d",
            "p_db1",
            "-c",
            "SELECT 1;",
        ])
        .output()
        .unwrap();
    assert!(
        !isolate_pg.status.success(),
        "Postgres isolation failed: p_user2 could access p_db1"
    );

    let temp_dir = tempfile::tempdir().unwrap();
    let remote_storage_dir = temp_dir.path().join("remote_storage");
    std::fs::create_dir_all(&remote_storage_dir).unwrap();
    let real_storage = Arc::new(RealFileStorageBackend {
        destination_dir: remote_storage_dir,
    });

    let mariadb_scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        mariadb_adapter.clone(),
        temp_dir.path().to_path_buf(),
        Compression::Zstd,
        None,
        real_storage.clone(),
        5,
    )
    .unwrap();
    let pg_scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        pg_adapter.clone(),
        temp_dir.path().to_path_buf(),
        Compression::Zstd,
        None,
        real_storage,
        5,
    )
    .unwrap();

    // MariaDB: Dump using scheduler
    let mariadb_receipt = mariadb_scheduler
        .execute_dump("m_db1", "mariadb_dump.zst")
        .await
        .unwrap();
    let mariadb_dump_path = std::path::PathBuf::from(mariadb_receipt.path_or_key);
    assert!(mariadb_dump_path.exists());

    // Decompress to verify content
    let mut mariadb_decompressed = String::new();
    let mut mariadb_decoder =
        zstd::stream::Decoder::new(std::fs::File::open(&mariadb_dump_path).unwrap()).unwrap();
    use std::io::Read;
    mariadb_decoder
        .read_to_string(&mut mariadb_decompressed)
        .unwrap();
    assert!(
        mariadb_decompressed.contains("mariadb_test_data"),
        "MariaDB dump missing expected data"
    );

    // Postgres: Dump using scheduler
    let pg_receipt = pg_scheduler
        .execute_dump("p_db1", "postgres_dump.zst")
        .await
        .unwrap();
    let pg_dump_path = std::path::PathBuf::from(pg_receipt.path_or_key);
    assert!(pg_dump_path.exists());

    let mut pg_decompressed = String::new();
    let mut pg_decoder =
        zstd::stream::Decoder::new(std::fs::File::open(&pg_dump_path).unwrap()).unwrap();
    pg_decoder.read_to_string(&mut pg_decompressed).unwrap();
    assert!(
        pg_decompressed.contains("postgres_test_data"),
        "Postgres dump missing expected data"
    );

    // MariaDB: Drop table and restore
    let drop_mariadb = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_file,
            "exec",
            "-T",
            "mariadb",
            "mysql",
            "-uroot",
            "-prootpassword",
            "m_db1",
            "-e",
            "DROP TABLE my_table;",
        ])
        .output()
        .expect("Failed to drop mariadb table");
    assert!(drop_mariadb.status.success());

    let uncompressed_mariadb_dump = temp_dir.path().join("mariadb_dump.sql");
    std::fs::write(&uncompressed_mariadb_dump, mariadb_decompressed).unwrap();

    use databases::adapter::RestorePlan;
    let mariadb_restore_plan = RestorePlan {
        db_name: DbName("m_db1".to_string()),
        input_path: uncompressed_mariadb_dump.to_string_lossy().to_string(),
    };
    mariadb_adapter
        .apply_restore(&mariadb_restore_plan)
        .await
        .unwrap();

    let verify_mariadb = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_file,
            "exec",
            "-T",
            "mariadb",
            "mysql",
            "-uroot",
            "-prootpassword",
            "m_db1",
            "-e",
            "SELECT data FROM my_table WHERE id=1;",
        ])
        .output()
        .expect("Failed to verify mariadb restore");
    assert!(
        String::from_utf8_lossy(&verify_mariadb.stdout).contains("mariadb_test_data"),
        "MariaDB restored data not found"
    );

    // Postgres: Drop table and restore
    let drop_pg = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_file,
            "exec",
            "-T",
            "postgres",
            "psql",
            "-U",
            "testuser",
            "-d",
            "p_db1",
            "-c",
            "DROP TABLE my_table;",
        ])
        .output()
        .expect("Failed to drop postgres table");
    assert!(drop_pg.status.success());

    let uncompressed_pg_dump = temp_dir.path().join("postgres_dump.sql");
    std::fs::write(&uncompressed_pg_dump, pg_decompressed).unwrap();

    let pg_restore_plan = RestorePlan {
        db_name: DbName("p_db1".to_string()),
        input_path: uncompressed_pg_dump.to_string_lossy().to_string(),
    };
    pg_adapter.apply_restore(&pg_restore_plan).await.unwrap();

    let verify_pg = Command::new("docker")
        .args([
            "compose",
            "-f",
            &compose_file,
            "exec",
            "-T",
            "postgres",
            "psql",
            "-U",
            "testuser",
            "-d",
            "p_db1",
            "-t",
            "-c",
            "SELECT data FROM my_table WHERE id=1;",
        ])
        .output()
        .expect("Failed to verify postgres restore");
    assert!(
        String::from_utf8_lossy(&verify_pg.stdout).contains("postgres_test_data"),
        "Postgres restored data not found"
    );

    // Teardown
    let _ = Command::new("docker")
        .args(["compose", "-f", &compose_file, "down", "-v"])
        .output()
        .expect("Failed to execute docker compose down");
}

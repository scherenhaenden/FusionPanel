use std::process::Command;
use std::time::Duration;

#[tokio::test]
async fn test_mariadb_and_postgres_dumps() {
    let docker_check = Command::new("docker").arg("info").output();
    if docker_check.is_err() || !docker_check.unwrap().status.success() {
        println!("BLOCKED: Docker is not available or not running.");
        return;
    }

    let compose_check = Command::new("docker")
        .args(["compose", "version"])
        .output();
    if compose_check.is_err() || !compose_check.unwrap().status.success() {
        println!("BLOCKED: docker compose is not available.");
        return;
    }

    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").unwrap();
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
        // The task environment blocks Docker overlayfs operations, which means we cannot start actual database containers.
        // We must log BLOCKED as required by the testing strategy for missing prerequisites.
        println!("BLOCKED: Failed to start docker compose (restricted environment): {}", err);
        return;
    }

    // Wait for databases to be ready
    let mut mariadb_ready = false;
    let mut postgres_ready = false;

    // Give it up to 60 seconds
    for _ in 0..12 {
        tokio::time::sleep(Duration::from_secs(5)).await;

        // Try executing a query in mariadb
        let mariadb_query = Command::new("docker")
            .args(["compose", "-f", &compose_file, "exec", "-T", "mariadb", "mysql", "-utestuser", "-ptestpassword", "-e", "SELECT 1;"])
            .output();

        if let Ok(output) = mariadb_query {
            if output.status.success() {
                mariadb_ready = true;
            }
        }

        // Try executing a query in postgres
        let pg_query = Command::new("docker")
            .args(["compose", "-f", &compose_file, "exec", "-T", "postgres", "psql", "-U", "testuser", "-d", "testdb", "-c", "SELECT 1;"])
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
        println!("BLOCKED: Databases did not become ready in time.");
        return;
    }

    // MariaDB: Create table and insert data
    let create_mariadb = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "mariadb", "mysql", "-utestuser", "-ptestpassword", "testdb", "-e",
              "CREATE TABLE my_table (id INT PRIMARY KEY, data VARCHAR(255)); INSERT INTO my_table (id, data) VALUES (1, 'mariadb_test_data');"])
        .output()
        .expect("Failed to create table in mariadb");
    assert!(create_mariadb.status.success(), "MariaDB table creation failed: {:?}", String::from_utf8_lossy(&create_mariadb.stderr));

    // Postgres: Create table and insert data
    let create_pg = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "postgres", "psql", "-U", "testuser", "-d", "testdb", "-c",
              "CREATE TABLE my_table (id INT PRIMARY KEY, data VARCHAR(255)); INSERT INTO my_table (id, data) VALUES (1, 'postgres_test_data');"])
        .output()
        .expect("Failed to create table in postgres");
    assert!(create_pg.status.success(), "Postgres table creation failed: {:?}", String::from_utf8_lossy(&create_pg.stderr));

    use integration_data::backup_scheduler::{DumpScheduler, Compression};
    use databases::adapter::{DatabaseConfig, DatabaseEngine, ConnectionStrategy, SecretString};
    use databases::mariadb::MariaDbAdapter;
    use databases::postgresql::PostgresAdapter;
    use std::sync::Arc;

    let mariadb_config = DatabaseConfig {
        engine: DatabaseEngine::MariaDB,
        strategy: ConnectionStrategy::Container { container_name: "mariadb".to_string(), network: None },
        host: "127.0.0.1".into(),
        port: 3306,
        admin_user: "testuser".into(),
        admin_password_ref: SecretString("testpassword".into()),
    };
    let mariadb_adapter = Arc::new(MariaDbAdapter::new(mariadb_config));
    
    let pg_config = DatabaseConfig {
        engine: DatabaseEngine::PostgreSQL,
        strategy: ConnectionStrategy::Container { container_name: "postgres".to_string(), network: None },
        host: "127.0.0.1".into(),
        port: 5432,
        admin_user: "testuser".into(),
        admin_password_ref: SecretString("rootpassword".into()), // Wait, compose says POSTGRES_PASSWORD: rootpassword
    };
    let pg_adapter = Arc::new(PostgresAdapter::new(pg_config));

    let temp_dir = tempfile::tempdir().unwrap();
    let mariadb_scheduler = DumpScheduler::new("0 0 0 * * * *", mariadb_adapter.clone(), temp_dir.path().to_path_buf(), Compression::Zstd).unwrap();
    let pg_scheduler = DumpScheduler::new("0 0 0 * * * *", pg_adapter.clone(), temp_dir.path().to_path_buf(), Compression::Zstd).unwrap();

    // MariaDB: Dump using scheduler
    let (mariadb_dump_path, mariadb_hash) = mariadb_scheduler.execute_dump("testdb", "mariadb_dump.zst").await.unwrap();
    assert!(mariadb_dump_path.exists());
    assert!(!mariadb_hash.is_empty());
    
    // Decompress to verify content
    let mut mariadb_decompressed = String::new();
    let mut mariadb_decoder = zstd::stream::Decoder::new(std::fs::File::open(&mariadb_dump_path).unwrap()).unwrap();
    use std::io::Read;
    mariadb_decoder.read_to_string(&mut mariadb_decompressed).unwrap();
    assert!(mariadb_decompressed.contains("mariadb_test_data"), "MariaDB dump missing expected data");

    // Postgres: Dump using scheduler
    let (pg_dump_path, pg_hash) = pg_scheduler.execute_dump("testdb", "postgres_dump.zst").await.unwrap();
    assert!(pg_dump_path.exists());
    assert!(!pg_hash.is_empty());

    let mut pg_decompressed = String::new();
    let mut pg_decoder = zstd::stream::Decoder::new(std::fs::File::open(&pg_dump_path).unwrap()).unwrap();
    pg_decoder.read_to_string(&mut pg_decompressed).unwrap();
    assert!(pg_decompressed.contains("postgres_test_data"), "Postgres dump missing expected data");

    // MariaDB: Drop table and restore
    let drop_mariadb = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "mariadb", "mysql", "-utestuser", "-ptestpassword", "testdb", "-e", "DROP TABLE my_table;"])
        .output()
        .expect("Failed to drop mariadb table");
    assert!(drop_mariadb.status.success());

    let uncompressed_mariadb_dump = temp_dir.path().join("mariadb_dump.sql");
    std::fs::write(&uncompressed_mariadb_dump, mariadb_decompressed).unwrap();
    
    use databases::adapter::{DatabaseAdapter, RestorePlan, DbName};
    let mariadb_restore_plan = RestorePlan {
        db_name: DbName("testdb".to_string()),
        input_path: uncompressed_mariadb_dump.to_string_lossy().to_string(),
    };
    mariadb_adapter.apply_restore(&mariadb_restore_plan).await.unwrap();

    let verify_mariadb = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "mariadb", "mysql", "-utestuser", "-ptestpassword", "testdb", "-e", "SELECT data FROM my_table WHERE id=1;"])
        .output()
        .expect("Failed to verify mariadb restore");
    assert!(String::from_utf8_lossy(&verify_mariadb.stdout).contains("mariadb_test_data"), "MariaDB restored data not found");

    // Postgres: Drop table and restore
    let drop_pg = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "postgres", "psql", "-U", "testuser", "-d", "testdb", "-c", "DROP TABLE my_table;"])
        .output()
        .expect("Failed to drop postgres table");
    assert!(drop_pg.status.success());

    let uncompressed_pg_dump = temp_dir.path().join("postgres_dump.sql");
    std::fs::write(&uncompressed_pg_dump, pg_decompressed).unwrap();
    
    let pg_restore_plan = RestorePlan {
        db_name: DbName("testdb".to_string()),
        input_path: uncompressed_pg_dump.to_string_lossy().to_string(),
    };
    pg_adapter.apply_restore(&pg_restore_plan).await.unwrap();

    let verify_pg = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "postgres", "psql", "-U", "testuser", "-d", "testdb", "-t", "-c", "SELECT data FROM my_table WHERE id=1;"])
        .output()
        .expect("Failed to verify postgres restore");
    assert!(String::from_utf8_lossy(&verify_pg.stdout).contains("postgres_test_data"), "Postgres restored data not found");

    // Teardown
    let _ = Command::new("docker")
        .args(["compose", "-f", &compose_file, "down", "-v"])
        .output()
        .expect("Failed to execute docker compose down");
}

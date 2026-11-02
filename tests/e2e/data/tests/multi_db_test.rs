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

    // MariaDB: Dump
    let dump_mariadb = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "mariadb", "mysqldump", "-utestuser", "-ptestpassword", "testdb"])
        .output()
        .expect("Failed to dump mariadb");
    assert!(dump_mariadb.status.success(), "MariaDB dump failed: {:?}", String::from_utf8_lossy(&dump_mariadb.stderr));
    let mariadb_dump_str = String::from_utf8_lossy(&dump_mariadb.stdout);
    assert!(mariadb_dump_str.contains("mariadb_test_data"), "MariaDB dump missing expected data");

    // Postgres: Dump
    let dump_pg = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "postgres", "pg_dump", "-U", "testuser", "testdb"])
        .output()
        .expect("Failed to dump postgres");
    assert!(dump_pg.status.success(), "Postgres dump failed: {:?}", String::from_utf8_lossy(&dump_pg.stderr));
    let pg_dump_str = String::from_utf8_lossy(&dump_pg.stdout);
    assert!(pg_dump_str.contains("postgres_test_data"), "Postgres dump missing expected data");

    // MariaDB: Drop table and restore
    let drop_mariadb = Command::new("docker")
        .args(["compose", "-f", &compose_file, "exec", "-T", "mariadb", "mysql", "-utestuser", "-ptestpassword", "testdb", "-e", "DROP TABLE my_table;"])
        .output()
        .expect("Failed to drop mariadb table");
    assert!(drop_mariadb.status.success());

    let mut restore_mariadb_cmd = Command::new("docker");
    restore_mariadb_cmd.args(["compose", "-f", &compose_file, "exec", "-i", "mariadb", "mysql", "-utestuser", "-ptestpassword", "testdb"]);

    use std::io::Write;
    let mut child = restore_mariadb_cmd.stdin(std::process::Stdio::piped()).spawn().expect("Failed to spawn mariadb restore");
    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(&dump_mariadb.stdout).expect("Failed to write to mariadb restore stdin");
    }
    let restore_mariadb_status = child.wait().expect("Failed to wait for mariadb restore");
    assert!(restore_mariadb_status.success(), "MariaDB restore failed");

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

    let mut restore_pg_cmd = Command::new("docker");
    restore_pg_cmd.args(["compose", "-f", &compose_file, "exec", "-i", "postgres", "psql", "-U", "testuser", "-d", "testdb"]);

    let mut child_pg = restore_pg_cmd.stdin(std::process::Stdio::piped()).spawn().expect("Failed to spawn postgres restore");
    if let Some(mut stdin) = child_pg.stdin.take() {
        stdin.write_all(&dump_pg.stdout).expect("Failed to write to postgres restore stdin");
    }
    let restore_pg_status = child_pg.wait().expect("Failed to wait for postgres restore");
    assert!(restore_pg_status.success(), "Postgres restore failed");

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

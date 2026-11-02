#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn test_disposable_db_lifecycle() {
    let sqlite_check = Command::new("sqlite3").arg("--version").output();

    if sqlite_check.is_err() || !sqlite_check.unwrap().status.success() {
        println!("BLOCKED: sqlite3 is not available. Skipping disposable DB test.");
        return;
    }

    let gzip_check = Command::new("gzip").arg("--version").output();

    if gzip_check.is_err() || !gzip_check.unwrap().status.success() {
        println!("BLOCKED: gzip is not available. Skipping disposable DB test.");
        return;
    }

    let temp_dir = env::temp_dir().join(format!("fusionpanel_db_test_{}", std::process::id()));
    if temp_dir.exists() {
        fs::remove_dir_all(&temp_dir).expect("Failed to clean up old temp dir");
    }
    fs::create_dir_all(&temp_dir).expect("Failed to create temp dir");

    let source_db = temp_dir.join("source.db");
    let dump_file = temp_dir.join("dump.sql.gz");
    let target_db = temp_dir.join("target.db");

    // 1. Create mock DB & insert sample tables
    let init_sql = "
        CREATE TABLE users (id INTEGER PRIMARY KEY, name TEXT);
        INSERT INTO users (name) VALUES ('Alice'), ('Bob'), ('Charlie');
        CREATE TABLE config (key TEXT PRIMARY KEY, value TEXT);
        INSERT INTO config (key, value) VALUES ('theme', 'dark');
    ";

    let mut init_cmd = Command::new("sqlite3")
        .arg(&source_db)
        .stdin(Stdio::piped())
        .spawn()
        .expect("Failed to spawn sqlite3 init");

    {
        let mut stdin = init_cmd.stdin.take().expect("Failed to open stdin");
        stdin
            .write_all(init_sql.as_bytes())
            .expect("Failed to write to sqlite3");
    }
    let init_status = init_cmd.wait().expect("Failed to wait on sqlite3 init");
    assert!(init_status.success(), "Failed to initialize source DB");

    // 2. Export compressed dump
    // sqlite3 source.db .dump | gzip > dump.sql.gz
    let mut dump_cmd = Command::new("sqlite3")
        .arg(&source_db)
        .arg(".dump")
        .stdout(Stdio::piped())
        .spawn()
        .expect("Failed to spawn sqlite3 dump");

    let dump_stdout = dump_cmd
        .stdout
        .take()
        .expect("Failed to capture dump stdout");

    let dump_file_handle = fs::File::create(&dump_file).expect("Failed to create dump file");

    let mut gzip_cmd = Command::new("gzip")
        .stdin(Stdio::from(dump_stdout))
        .stdout(Stdio::from(dump_file_handle))
        .spawn()
        .expect("Failed to spawn gzip");

    let gzip_status = gzip_cmd.wait().expect("Failed to wait on gzip");
    assert!(gzip_status.success(), "Failed to create compressed dump");
    let dump_status = dump_cmd.wait().expect("Failed to wait on dump");
    assert!(dump_status.success(), "Failed to create uncompressed dump");

    // 3. Restore into fresh disposable DB
    // zcat dump.sql.gz | sqlite3 target.db
    let zcat_cmd_res = Command::new("zcat")
        .arg(&dump_file)
        .stdout(Stdio::piped())
        .spawn();

    let mut zcat_cmd = match zcat_cmd_res {
        Ok(cmd) => cmd,
        Err(_) => {
            // fallback if zcat is not available, use gzip -d -c
            Command::new("gzip")
                .arg("-d")
                .arg("-c")
                .arg(&dump_file)
                .stdout(Stdio::piped())
                .spawn()
                .expect("Failed to spawn gzip decompress")
        }
    };

    let zcat_stdout = zcat_cmd
        .stdout
        .take()
        .expect("Failed to capture zcat stdout");

    let mut restore_cmd = Command::new("sqlite3")
        .arg(&target_db)
        .stdin(Stdio::from(zcat_stdout))
        .spawn()
        .expect("Failed to spawn sqlite3 restore");

    let restore_status = restore_cmd
        .wait()
        .expect("Failed to wait on sqlite3 restore");
    assert!(restore_status.success(), "Failed to restore target DB");
    let zcat_status = zcat_cmd.wait().expect("Failed to wait on zcat");
    assert!(zcat_status.success(), "Failed to decompress dump");

    // 4. Verify row counts and checksums
    let check_users = Command::new("sqlite3")
        .arg(&target_db)
        .arg("SELECT count(*) FROM users;")
        .output()
        .expect("Failed to query target DB");

    assert!(check_users.status.success());
    let users_count = String::from_utf8_lossy(&check_users.stdout)
        .trim()
        .to_string();
    assert_eq!(users_count, "3");

    let check_config = Command::new("sqlite3")
        .arg(&target_db)
        .arg("SELECT value FROM config WHERE key='theme';")
        .output()
        .expect("Failed to query target DB");

    assert!(check_config.status.success());
    let theme_val = String::from_utf8_lossy(&check_config.stdout)
        .trim()
        .to_string();
    assert_eq!(theme_val, "dark");

    // Checksum verification using sqlite3's internal .sha3sum command (if available, else skip hash, check md5sum of dumps)
    let source_dump = Command::new("sqlite3")
        .arg(&source_db)
        .arg(".dump")
        .output()
        .expect("Failed to dump source for checksum");
    let target_dump = Command::new("sqlite3")
        .arg(&target_db)
        .arg(".dump")
        .output()
        .expect("Failed to dump target for checksum");

    assert_eq!(source_dump.stdout, target_dump.stdout, "Dumps do not match");

    // Cleanup
    fs::remove_dir_all(&temp_dir).expect("Failed to cleanup temp dir");
}

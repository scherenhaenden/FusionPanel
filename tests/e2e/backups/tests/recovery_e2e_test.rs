#![forbid(unsafe_code)]

use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::Command;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

// Copyright Notice for AGPL-3.0-only compliance
// (c) FusionPanel

#[test]
fn test_backup_and_recovery_e2e() {
    let temp_dir = std::env::temp_dir().join(format!(
        "fusionpanel_test_{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis()
    ));
    fs::create_dir_all(&temp_dir).unwrap();

    let data_dir = temp_dir.join("data");
    fs::create_dir_all(&data_dir).unwrap();
    fs::write(data_dir.join("file1.txt"), "hello world").unwrap();
    fs::write(data_dir.join("file2.txt"), "test data").unwrap();

    let db_dump_path = temp_dir.join("db_dump.sql");
    fs::write(&db_dump_path, "CREATE TABLE test (id INT);").unwrap();

    // Mock S3 multipart server
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();

    let server_thread = thread::spawn(move || {
        if let Ok((mut stream, _)) = listener.accept() {
            let mut buf = [0; 1024];
            let _ = stream.read(&mut buf);
            // Respond with standard 200 OK for mock
            let response = "HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
            let _ = stream.write_all(response.as_bytes());
        }
    });

    // Simulate Backup Generation
    let backup_tar = std::env::temp_dir().join(format!(
        "fusionpanel_test_{}_backup.tar.zst",
        SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_millis()
    ));
    
    use backups::backup::BackupJob;
    let job = BackupJob::new(&temp_dir);
    let manifest = job.execute(&backup_tar).expect("Failed to execute backup job");
    assert!(backup_tar.exists());

    // Simulate S3 upload via curl to our mock server
    let mut check_curl = std::process::Command::new("sh");
    check_curl.args(["-c", "command -v curl"]);
    if check_curl.output().map(|o| o.status.success()).unwrap_or(false) {
        let s3_upload_status = Command::new("curl")
            .arg("-X")
            .arg("POST")
            .arg("-d")
            .arg(format!("@{}", backup_tar.display()))
            .arg(format!("http://127.0.0.1:{}/upload?uploadId=mock123", port))
            .status()
            .expect("Failed to run curl");

        assert!(s3_upload_status.success());
    } else {
        println!("BLOCKED: curl not available in environment");
    }
    server_thread.join().unwrap();

    // Simulate Recovery
    let restore_dir = temp_dir.join("restore");
    fs::create_dir_all(&restore_dir).unwrap();

    use backups::restore::restore_archive;
    let restore_manifest = restore_archive(&backup_tar, &restore_dir).expect("Failed to restore archive");
    assert_eq!(restore_manifest.version, manifest.version);
    assert_eq!(restore_manifest.backup_id, manifest.backup_id);

    // Recovery Integrity Assertion
    let restored_file1 = fs::read_to_string(restore_dir.join("data").join("file1.txt")).unwrap();
    assert_eq!(restored_file1, "hello world");

    let restored_file2 = fs::read_to_string(restore_dir.join("data").join("file2.txt")).unwrap();
    assert_eq!(restored_file2, "test data");

    let restored_db = fs::read_to_string(restore_dir.join("db_dump.sql")).unwrap();
    assert_eq!(restored_db, "CREATE TABLE test (id INT);");

    // Cleanup
    fs::remove_dir_all(&temp_dir).unwrap();
    let _ = fs::remove_file(&backup_tar);
}

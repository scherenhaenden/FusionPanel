#![forbid(unsafe_code)]

use containers::log_rotator::LogRotator;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use tempfile::tempdir;

#[test]
fn test_no_rotation_when_under_limit() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("container.log");
    fs::write(&log_path, "a".repeat(10)).unwrap();

    let rotator = LogRotator::new(50, 3);
    rotator.rotate(&log_path).unwrap();

    assert!(log_path.exists());
    assert!(!dir.path().join("container.log.1").exists());
    assert!(!dir.path().join("container.log.1.gz").exists());

    // File content should be unmodified
    let content = fs::read_to_string(&log_path).unwrap();
    assert_eq!(content, "a".repeat(10));
}

#[test]
fn test_rotation_when_over_limit() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("container.log");
    fs::write(&log_path, "a".repeat(100)).unwrap();

    let rotator = LogRotator::new(50, 3);
    rotator.rotate(&log_path).unwrap();

    assert!(log_path.exists());
    // New log file should be empty
    assert_eq!(fs::metadata(&log_path).unwrap().len(), 0);

    // Check permissions are 0600
    let perms = fs::metadata(&log_path).unwrap().permissions();
    assert_eq!(perms.mode() & 0o777, 0o600);

    // Rotated file should exist
    assert!(dir.path().join("container.log.1.gz").exists());
    assert!(!dir.path().join("container.log.1").exists()); // uncompressed should not exist
}

#[test]
fn test_retention_limit() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("container.log");
    let rotator = LogRotator::new(50, 2);

    // 1st rotation
    fs::write(&log_path, "a".repeat(100)).unwrap();
    rotator.rotate(&log_path).unwrap();
    assert!(dir.path().join("container.log.1.gz").exists());

    // 2nd rotation
    fs::write(&log_path, "b".repeat(100)).unwrap();
    rotator.rotate(&log_path).unwrap();
    assert!(dir.path().join("container.log.1.gz").exists());
    assert!(dir.path().join("container.log.2.gz").exists());

    // 3rd rotation (should remove oldest)
    fs::write(&log_path, "c".repeat(100)).unwrap();
    rotator.rotate(&log_path).unwrap();
    assert!(dir.path().join("container.log.1.gz").exists());
    assert!(dir.path().join("container.log.2.gz").exists());
    assert!(!dir.path().join("container.log.3.gz").exists()); // not created due to limit = 2
}

#[test]
fn test_file_not_found() {
    let dir = tempdir().unwrap();
    let log_path = dir.path().join("nonexistent.log");

    let rotator = LogRotator::new(50, 3);
    assert!(rotator.rotate(&log_path).is_ok());
}

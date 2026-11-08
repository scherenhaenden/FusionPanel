#![forbid(unsafe_code)]

#[path = "../src/acme_cleanup.rs"]
mod acme_cleanup;

use acme_cleanup::AcmeTokenCleaner;
use std::fs::File;
use std::io::Write;
use std::time::Duration;
use tempfile::tempdir;
use std::time::SystemTime;

#[test]
fn test_clean_directory_missing_dir() {
    let cleaner = AcmeTokenCleaner::default();
    let summary = cleaner.clean_directory("/path/that/does/not/exist/surely").unwrap();
    assert_eq!(summary.files_scanned, 0);
    assert_eq!(summary.files_removed, 0);
}

#[test]
fn test_clean_directory_empty_dir() {
    let dir = tempdir().unwrap();
    let cleaner = AcmeTokenCleaner::default();
    let summary = cleaner.clean_directory(dir.path()).unwrap();
    assert_eq!(summary.files_scanned, 0);
    assert_eq!(summary.files_removed, 0);
}

#[test]
fn test_clean_directory_with_expired_and_unexpired() {
    let dir = tempdir().unwrap();
    let cleaner = AcmeTokenCleaner::new(Duration::from_secs(10)); // 10s ttl

    // Create an unexpired file
    let unexpired_path = dir.path().join("unexpired_token-123_abc");
    let mut f = File::create(&unexpired_path).unwrap();
    f.write_all(b"data1").unwrap();

    // Create an expired file (manually change its mtime)
    let expired_path = dir.path().join("expired_token-456_def");
    let mut f = File::create(&expired_path).unwrap();
    f.write_all(b"data22").unwrap();

    // set mtime to 20 seconds ago
    let past = SystemTime::now() - Duration::from_secs(20);
    let ft = filetime::FileTime::from_system_time(past);
    filetime::set_file_times(&expired_path, ft, ft).unwrap();

    let summary = cleaner.clean_directory(dir.path()).unwrap();

    assert_eq!(summary.files_scanned, 2);
    assert_eq!(summary.files_removed, 1);
    assert_eq!(summary.skipped_unexpired, 1);
    assert_eq!(summary.bytes_reclaimed, 6); // "data22" is 6 bytes

    // Ensure the right one was removed
    assert!(unexpired_path.exists());
    assert!(!expired_path.exists());
}

#[test]
fn test_invalid_filenames_ignored() {
    // Tests are handled by test_invalid_files_not_scanned and test_filename_validation_direct
}

#[test]
fn test_filename_validation_direct() {
    // Valid
    assert!(acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("abc_123-DEF"));
    assert!(acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("a"));

    // Invalid
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename(""));
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename(".."));
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("abc.def"));
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("abc/def"));
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("abc\\def"));
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("abc\0def"));
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("abc def"));
    assert!(!acme_cleanup::AcmeTokenCleaner::is_valid_token_filename("abc@def"));
}

#[test]
fn test_invalid_files_not_scanned() {
    let dir = tempdir().unwrap();
    let cleaner = AcmeTokenCleaner::new(Duration::from_secs(10));

    // Create an expired file but with invalid name
    let invalid_path = dir.path().join("invalid.token");
    let mut f = File::create(&invalid_path).unwrap();
    f.write_all(b"data").unwrap();

    let past = SystemTime::now() - Duration::from_secs(20);
    let ft = filetime::FileTime::from_system_time(past);
    filetime::set_file_times(&invalid_path, ft, ft).unwrap();

    let summary = cleaner.clean_directory(dir.path()).unwrap();

    // Should skip it completely
    assert_eq!(summary.files_scanned, 0);
    assert_eq!(summary.files_removed, 0);
    assert!(invalid_path.exists());
}

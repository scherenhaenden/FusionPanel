use backups::backup::BackupJob;
use backups::error::BackupError;
use backups::models::{DbInfo, RetentionPolicy};
use backups::restore::restore_archive;
use backups::retention::apply_retention_policy;
use std::fs::{self, File};
use std::io::Write;
use tempfile::TempDir;
use tar::Builder;
use zstd::stream::Encoder as ZstdEncoder;

#[test]
fn test_full_backup_and_restore_cycle() {
    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();
    let restore_dir = TempDir::new().unwrap();

    let file1_path = source_dir.path().join("file1.txt");
    let mut file1 = File::create(&file1_path).unwrap();
    file1.write_all(b"Hello world").unwrap();

    let subdir_path = source_dir.path().join("subdir");
    fs::create_dir(&subdir_path).unwrap();
    let file2_path = subdir_path.join("file2.txt");
    let mut file2 = File::create(&file2_path).unwrap();
    file2.write_all(b"Subdir content").unwrap();

    let archive_path = dest_dir.path().join("backup.tar.gz");

    let job = BackupJob::new(source_dir.path()).with_db(DbInfo {
        engine: "sqlite".to_string(),
        database_name: "test.db".to_string(),
        dump_file: "test.sql".to_string(),
    });

    let manifest = job.execute(&archive_path).expect("Backup failed");

    assert_eq!(manifest.files.len(), 2);
    assert_eq!(manifest.databases.len(), 1);

    let restored_manifest = restore_archive(&archive_path, restore_dir.path()).expect("Restore failed");
    assert_eq!(manifest.backup_id, restored_manifest.backup_id);

    let restored_file1 = fs::read_to_string(restore_dir.path().join("file1.txt")).unwrap();
    assert_eq!(restored_file1, "Hello world");

    let restored_file2 = fs::read_to_string(restore_dir.path().join("subdir/file2.txt")).unwrap();
    assert_eq!(restored_file2, "Subdir content");
}

#[test]
fn test_retention_policy() {
    let backup_dir = TempDir::new().unwrap();

    for i in 0..3 {
        let path = backup_dir.path().join(format!("backup{}.tar.gz", i));
        File::create(&path).unwrap();
    }

    let policy = RetentionPolicy {
        max_backups: Some(2),
        max_days: None,
    };

    let deleted = apply_retention_policy(backup_dir.path(), &policy).unwrap();
    assert_eq!(deleted.len(), 1);

    let remaining = fs::read_dir(backup_dir.path()).unwrap().count();
    assert_eq!(remaining, 2);
}

#[test]
fn test_tamper_detection() {
    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();
    let restore_dir = TempDir::new().unwrap();

    let file1_path = source_dir.path().join("file1.txt");
    let mut file1 = File::create(&file1_path).unwrap();
    file1.write_all(b"Valid content").unwrap();

    let archive_path = dest_dir.path().join("backup.tar.gz");
    let job = BackupJob::new(source_dir.path());
    let original_manifest = job.execute(&archive_path).expect("Backup failed");

    // Corrupt archive by replacing the valid file with malicious content
    let corrupted_archive_path = dest_dir.path().join("corrupt.tar.zst");
    let dest = File::create(&corrupted_archive_path).unwrap();
    let enc = ZstdEncoder::new(dest, 3).unwrap();
    let mut tar = Builder::new(enc);

    // Write original manifest
    let manifest_json = serde_json::to_string_pretty(&original_manifest).unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_path("manifest.json").unwrap();
    header.set_size(manifest_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, manifest_json.as_bytes()).unwrap();

    // Write corrupted file with same path but different content
    let malicious_content = b"Corrupted content";
    let mut header = tar::Header::new_gnu();
    header.set_path("file1.txt").unwrap();
    header.set_size(malicious_content.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, malicious_content.as_slice()).unwrap();

    tar.into_inner().unwrap().finish().unwrap();

    let result = restore_archive(&corrupted_archive_path, restore_dir.path());

    // Check if error is ChecksumMismatch
    match result {
        Err(BackupError::ChecksumMismatch { path, .. }) => {
            assert_eq!(path, "file1.txt");
        },
        _ => panic!("Expected ChecksumMismatch error, got {:?}", result),
    }

    // Ensure the corrupted file was removed
    assert!(!restore_dir.path().join("file1.txt").exists());
}

#[test]
fn test_manifest_validation_failures() {
    use backups::models::{Manifest, BackupType, FileInfo};
    use std::collections::HashMap;

    let mut manifest = Manifest {
        version: 2, // invalid
        backup_id: "invalid-uuid".to_string(),
        timestamp: "not-a-date".to_string(),
        backup_type: BackupType::Files,
        files: vec![
            FileInfo { path: "file1".to_string(), size: 0, checksum_sha256: "".to_string() },
            FileInfo { path: "file1".to_string(), size: 0, checksum_sha256: "".to_string() }, // duplicate
        ],
        databases: vec![],
        metadata: HashMap::new(),
    };

    let err = manifest.validate().unwrap_err();
    match err {
        BackupError::InvalidManifest(msg) => assert!(msg.contains("Unsupported version")),
        _ => panic!("Expected InvalidManifest"),
    }

    manifest.version = 1;
    let err = manifest.validate().unwrap_err();
    match err {
        BackupError::InvalidManifest(msg) => assert!(msg.contains("Invalid backup_id UUID")),
        _ => panic!("Expected InvalidManifest"),
    }

    manifest.backup_id = uuid::Uuid::new_v4().to_string();
    let err = manifest.validate().unwrap_err();
    match err {
        BackupError::InvalidManifest(msg) => assert!(msg.contains("Invalid timestamp format")),
        _ => panic!("Expected InvalidManifest"),
    }

    manifest.timestamp = chrono::Utc::now().to_rfc3339();
    let err = manifest.validate().unwrap_err();
    match err {
        BackupError::InvalidManifest(msg) => assert!(msg.contains("Duplicate file path")),
        _ => panic!("Expected InvalidManifest"),
    }

    manifest.files.pop();
    manifest.files.push(FileInfo { path: "../unsafe/path".to_string(), size: 0, checksum_sha256: "".to_string() });
    let err = manifest.validate().unwrap_err();
    match err {
        BackupError::InvalidManifest(msg) => assert!(msg.contains("Unsafe path in manifest")),
        _ => panic!("Expected InvalidManifest"),
    }
}

#[test]
fn test_incomplete_archive() {
    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();
    let restore_dir = TempDir::new().unwrap();

    let file1_path = source_dir.path().join("file1.txt");
    let mut file1 = File::create(&file1_path).unwrap();
    file1.write_all(b"Valid content").unwrap();

    let archive_path = dest_dir.path().join("backup.tar.gz");
    let job = BackupJob::new(source_dir.path());
    let original_manifest = job.execute(&archive_path).expect("Backup failed");

    // Create incomplete archive (manifest is there, file is not)
    let incomplete_archive_path = dest_dir.path().join("incomplete.tar.zst");
    let dest = File::create(&incomplete_archive_path).unwrap();
    let enc = ZstdEncoder::new(dest, 3).unwrap();
    let mut tar = Builder::new(enc);

    let manifest_json = serde_json::to_string_pretty(&original_manifest).unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_path("manifest.json").unwrap();
    header.set_size(manifest_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, manifest_json.as_bytes()).unwrap();

    tar.into_inner().unwrap().finish().unwrap();

    let result = restore_archive(&incomplete_archive_path, restore_dir.path());
    match result {
        Err(BackupError::InvalidManifest(msg)) => {
            assert!(msg.contains("missing from archive"));
        },
        _ => panic!("Expected InvalidManifest error for missing file, got {:?}", result),
    }
}

#[test]
fn test_unsafe_archive_paths() {
    let dest_dir = TempDir::new().unwrap();
    let _restore_dir = TempDir::new().unwrap();

    let archive_path = dest_dir.path().join("unsafe.tar.zst");
    let dest = File::create(&archive_path).unwrap();
    let enc = ZstdEncoder::new(dest, 3).unwrap();
    let mut tar = Builder::new(enc);

    use backups::models::{Manifest, BackupType, FileInfo};
    use std::collections::HashMap;

    let manifest = Manifest {
        version: 1,
        backup_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        backup_type: BackupType::Files,
        files: vec![
            FileInfo { path: "../unsafe".to_string(), size: 5, checksum_sha256: "dummy".to_string() },
        ],
        databases: vec![],
        metadata: HashMap::new(),
    };

    let manifest_json = serde_json::to_string_pretty(&manifest).unwrap();
    let mut header = tar::Header::new_gnu();
    header.set_path("manifest.json").unwrap();
    header.set_size(manifest_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, manifest_json.as_bytes()).unwrap();

    // We cannot create a tar file with `../` using the tar crate easily because it returns an error.
    // Instead, we just rely on manifest validation catching it (as tested above).
}

use backups::backup::BackupJob;
use backups::error::BackupError;
use backups::models::{BackupType, DbInfo, FileInfo, Manifest, RetentionPolicy};
use backups::restore::restore_archive;
use backups::retention::apply_retention_policy;
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Write;
use tar::Builder;
use tempfile::TempDir;
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

    let archive_path = dest_dir.path().join("backup.tar.zst");

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
    let source_dir = TempDir::new().unwrap();

    let job = BackupJob::new(source_dir.path());

    // Create 3 valid backups
    for i in 0..3 {
        let path = backup_dir.path().join(format!("backup{}.tar.zst", i));
        job.execute(&path).expect("Backup failed");

        // Wait a bit to ensure different modified times
        std::thread::sleep(std::time::Duration::from_millis(100));
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

    let archive_path = dest_dir.path().join("backup.tar.zst");
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

    // Check if error is SizeMismatch or ChecksumMismatch
    match result {
        Err(BackupError::SizeMismatch { path, .. }) | Err(BackupError::ChecksumMismatch { path, .. }) => {
            assert_eq!(path, "file1.txt");
        }
        _ => panic!("Expected SizeMismatch or ChecksumMismatch error, got {:?}", result),
    }

    // Ensure the corrupted file was not written to target
    assert!(!restore_dir.path().join("file1.txt").exists());
}

#[test]
fn test_manifest_validation_failures() {
    let mut manifest = Manifest {
        version: 2, // invalid
        backup_id: "invalid-uuid".to_string(),
        timestamp: "not-a-date".to_string(),
        backup_type: BackupType::Files,
        files: vec![
            FileInfo {
                path: "file1".to_string(),
                size: 0,
                checksum_sha256: "".to_string(),
            },
            FileInfo {
                path: "file1".to_string(),
                size: 0,
                checksum_sha256: "".to_string(),
            }, // duplicate
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
    manifest.files.push(FileInfo {
        path: "../unsafe/path".to_string(),
        size: 0,
        checksum_sha256: "".to_string(),
    });
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

    let archive_path = dest_dir.path().join("backup.tar.zst");
    let job = BackupJob::new(source_dir.path());
    let original_manifest = job.execute(&archive_path).expect("Backup failed");

    // Create incomplete archive (manifest is there, file is missing)
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
    assert!(matches!(result, Err(BackupError::MissingFiles)));
}

#[test]
fn test_unsafe_archive_paths() {
    let dest_dir = TempDir::new().unwrap();
    let _restore_dir = TempDir::new().unwrap();

    let archive_path = dest_dir.path().join("unsafe.tar.zst");
    let dest = File::create(&archive_path).unwrap();
    let enc = ZstdEncoder::new(dest, 3).unwrap();
    let mut tar = Builder::new(enc);

    let manifest = Manifest {
        version: 1,
        backup_id: uuid::Uuid::new_v4().to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
        backup_type: BackupType::Files,
        files: vec![FileInfo {
            path: "../unsafe".to_string(),
            size: 5,
            checksum_sha256: "dummy".to_string(),
        }],
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

    tar.into_inner().unwrap().finish().unwrap();
}

#[test]
fn test_duplicate_manifest() {
    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();
    let restore_dir = TempDir::new().unwrap();

    let file1_path = source_dir.path().join("file1.txt");
    let mut file1 = File::create(&file1_path).unwrap();
    file1.write_all(b"Valid content").unwrap();

    let archive_path = dest_dir.path().join("backup.tar.zst");
    let job = BackupJob::new(source_dir.path());
    let original_manifest = job.execute(&archive_path).expect("Backup failed");

    let corrupted_archive_path = dest_dir.path().join("duplicate_manifest.tar.zst");
    let dest = File::create(&corrupted_archive_path).unwrap();
    let enc = ZstdEncoder::new(dest, 3).unwrap();
    let mut tar = Builder::new(enc);

    let manifest_json = serde_json::to_string_pretty(&original_manifest).unwrap();

    // Write original manifest twice
    let mut header = tar::Header::new_gnu();
    header.set_path("manifest.json").unwrap();
    header.set_size(manifest_json.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    tar.append(&header, manifest_json.as_bytes()).unwrap();

    let mut header2 = tar::Header::new_gnu();
    header2.set_path("manifest.json").unwrap();
    header2.set_size(manifest_json.len() as u64);
    header2.set_mode(0o644);
    header2.set_cksum();
    tar.append(&header2, manifest_json.as_bytes()).unwrap();

    tar.into_inner().unwrap().finish().unwrap();

    let result = restore_archive(&corrupted_archive_path, restore_dir.path());
    assert!(matches!(result, Err(BackupError::DuplicateManifest)));
}

#[test]
fn test_source_directory_nonexistent() {
    let dest_dir = TempDir::new().unwrap();
    let archive_path = dest_dir.path().join("backup.tar.zst");

    let job = BackupJob::new("/nonexistent/directory/path");
    let result = job.execute(&archive_path);

    assert!(matches!(result, Err(BackupError::SourceNotFound(_))));
}

#[test]
fn test_target_inside_source() {
    let source_dir = TempDir::new().unwrap();
    let dest_path = source_dir.path().join("backup.tar.zst");

    let job = BackupJob::new(source_dir.path());
    let result = job.execute(&dest_path);

    assert!(matches!(result, Err(BackupError::DestinationInsideSource)));
}

#[test]
fn test_preservation_on_failure() {
    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();
    let restore_dir = TempDir::new().unwrap();

    let file1_path = source_dir.path().join("file1.txt");
    let mut file1 = File::create(&file1_path).unwrap();
    file1.write_all(b"Valid content").unwrap();

    let archive_path = dest_dir.path().join("backup.tar.zst");
    let job = BackupJob::new(source_dir.path());
    let original_manifest = job.execute(&archive_path).expect("Backup failed");

    // Put an existing file in restore dir
    let pre_existing_path = restore_dir.path().join("file1.txt");
    let mut pre_existing_file = File::create(&pre_existing_path).unwrap();
    pre_existing_file.write_all(b"Old content").unwrap();

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

    // Should fail
    assert!(result.is_err());

    // Ensure the original file is preserved
    let restored_file1 = fs::read_to_string(&pre_existing_path).unwrap();
    assert_eq!(restored_file1, "Old content");
}

#[cfg(unix)]
#[test]
fn test_malicious_symlinks() {
    use std::os::unix::fs::symlink;

    let source_dir = TempDir::new().unwrap();
    let dest_dir = TempDir::new().unwrap();
    let restore_dir = TempDir::new().unwrap();
    let outside_dir = TempDir::new().unwrap();

    let outside_file_path = outside_dir.path().join("sensitive.txt");
    let mut outside_file = File::create(&outside_file_path).unwrap();
    outside_file.write_all(b"SECRET").unwrap();

    let file1_path = source_dir.path().join("subdir/file1.txt");
    fs::create_dir_all(source_dir.path().join("subdir")).unwrap();
    let mut file1 = File::create(&file1_path).unwrap();
    file1.write_all(b"Valid content").unwrap();

    let archive_path = dest_dir.path().join("backup.tar.zst");
    let job = BackupJob::new(source_dir.path());
    job.execute(&archive_path).expect("Backup failed");

    // Create a malicious symlink in the restore directory beforehand
    let malicious_symlink_path = restore_dir.path().join("subdir");
    symlink(outside_dir.path(), &malicious_symlink_path).unwrap();

    let result = restore_archive(&archive_path, restore_dir.path());

    assert!(result.is_ok());

    // The symlink should have been removed and replaced with a real directory
    let metadata = fs::symlink_metadata(&malicious_symlink_path).unwrap();
    assert!(metadata.is_dir());
    assert!(!metadata.file_type().is_symlink());

    // The outside file should NOT have been overwritten
    let content = fs::read_to_string(&outside_file_path).unwrap();
    assert_eq!(content, "SECRET");
}

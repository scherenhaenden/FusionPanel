#![forbid(unsafe_code)]

use backup_destinations::local::LocalStorageBackend;
use backup_destinations::traits::{StorageBackend, StorageError};
use sha2::Digest;
use tempfile::tempdir;

#[tokio::test]
async fn test_local_upload_download() {
    let dir = tempdir().unwrap();
    let backend = LocalStorageBackend::new(dir.path().to_path_buf()).unwrap();

    let res = backend.upload("test.txt", vec![1, 2, 3]).await;
    assert!(res.is_ok());

    let res = backend.download("test.txt").await;
    assert!(res.is_ok());
    assert_eq!(res.unwrap(), vec![1, 2, 3]);
}

#[tokio::test]
async fn test_local_permissions() {
    let dir = tempdir().unwrap();
    let backend = LocalStorageBackend::new(dir.path().to_path_buf()).unwrap();

    backend.upload("test.txt", vec![1, 2, 3]).await.unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = std::fs::metadata(dir.path().join("test.txt")).unwrap();
        assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
    }
}

#[tokio::test]
async fn test_local_path_escape() {
    let dir = tempdir().unwrap();
    let backend = LocalStorageBackend::new(dir.path().to_path_buf()).unwrap();

    let res = backend.upload("../test.txt", vec![1, 2, 3]).await;
    assert_eq!(res.unwrap_err(), StorageError::PermissionDenied);
}

#[tokio::test]
async fn test_local_list_stat_delete() {
    let dir = tempdir().unwrap();
    let backend = LocalStorageBackend::new(dir.path().to_path_buf()).unwrap();

    backend.upload("folder/a.txt", vec![1]).await.unwrap();
    backend.upload("folder/b.txt", vec![1, 2]).await.unwrap();

    let list = backend.list("folder/").await.unwrap();
    assert_eq!(list, vec!["folder/a.txt", "folder/b.txt"]);

    let stat = backend.stat("folder/b.txt").await.unwrap();
    assert_eq!(stat.size, 2);

    backend.delete("folder/a.txt").await.unwrap();
    let list = backend.list("folder/").await.unwrap();
    assert_eq!(list, vec!["folder/b.txt"]);
}

#[tokio::test]
async fn test_local_verify_integrity() {
    let dir = tempdir().unwrap();
    let backend = LocalStorageBackend::new(dir.path().to_path_buf()).unwrap();

    let data = b"some backup data";
    backend.upload("backup.bin", data.to_vec()).await.unwrap();

    let mut hasher = sha2::Sha256::new();
    sha2::Digest::update(&mut hasher, data);
    let expected_hash = hex::encode(hasher.finalize());

    assert!(backend
        .verify_integrity("backup.bin", &expected_hash)
        .await
        .unwrap());
    assert!(!backend
        .verify_integrity("backup.bin", "invalid_hash")
        .await
        .unwrap());
}

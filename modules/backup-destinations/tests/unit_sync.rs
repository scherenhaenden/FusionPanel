#![forbid(unsafe_code)]

use backup_destinations::sync::{
    BackupManifest, RetentionPolicy, StorageDest, SyncEngine, SyncError,
};
use chrono::{TimeZone, Utc};
use sha2::{Digest, Sha256};
use std::cell::RefCell;

struct MockStorage {
    archives: RefCell<Vec<String>>,
    uploaded: RefCell<Vec<(String, Vec<u8>)>>,
    deleted: RefCell<Vec<String>>,
}

impl MockStorage {
    fn new(existing: Vec<&str>) -> Self {
        Self {
            archives: RefCell::new(existing.into_iter().map(|s| s.to_string()).collect()),
            uploaded: RefCell::new(Vec::new()),
            deleted: RefCell::new(Vec::new()),
        }
    }
}

impl StorageDest for &MockStorage {
    fn upload_archive(&self, id: &str, data: &[u8]) -> Result<(), SyncError> {
        self.uploaded
            .borrow_mut()
            .push((id.to_string(), data.to_vec()));
        self.archives.borrow_mut().push(id.to_string());
        Ok(())
    }

    fn delete_archive(&self, id: &str) -> Result<(), SyncError> {
        self.deleted.borrow_mut().push(id.to_string());
        self.archives.borrow_mut().retain(|x| x != id);
        Ok(())
    }

    fn list_archives(&self) -> Result<Vec<String>, SyncError> {
        Ok(self.archives.borrow().clone())
    }
}

fn compute_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    hex::encode(hasher.finalize())
}

#[test]
fn test_upload_and_verify_success() {
    let data = b"some archive data";
    let manifest = BackupManifest {
        id: "backup-1".to_string(),
        timestamp: Utc::now(),
        checksum: compute_hash(data),
        archive_size: data.len() as u64,
    };

    let storage = MockStorage::new(vec![]);
    let engine = SyncEngine::new(&storage);

    let result = engine.upload_and_verify(&manifest, data);
    assert!(result.is_ok());
    assert_eq!(storage.uploaded.borrow().len(), 1);
    assert_eq!(storage.uploaded.borrow()[0].0, "backup-1");
}

#[test]
fn test_upload_and_verify_failure() {
    let data = b"some archive data";
    let manifest = BackupManifest {
        id: "backup-1".to_string(),
        timestamp: Utc::now(),
        checksum: "invalid_hash".to_string(),
        archive_size: data.len() as u64,
    };

    let storage = MockStorage::new(vec![]);
    let engine = SyncEngine::new(&storage);

    let result = engine.upload_and_verify(&manifest, data);
    assert!(matches!(result, Err(SyncError::IntegrityError(_, _, _))));
    assert_eq!(storage.uploaded.borrow().len(), 0);
}

#[test]
fn test_reconcile() {
    let local_manifests = vec![
        BackupManifest {
            id: "keep-1".to_string(),
            timestamp: Utc::now(),
            checksum: "".to_string(),
            archive_size: 0,
        },
        BackupManifest {
            id: "keep-2".to_string(),
            timestamp: Utc::now(),
            checksum: "".to_string(),
            archive_size: 0,
        },
    ];

    let storage = MockStorage::new(vec!["keep-1", "orphan-1", "orphan-2"]);
    let engine = SyncEngine::new(&storage);

    let deleted = engine.reconcile(&local_manifests).unwrap();
    assert_eq!(deleted.len(), 2);
    assert!(deleted.contains(&"orphan-1".to_string()));
    assert!(deleted.contains(&"orphan-2".to_string()));
    assert_eq!(storage.deleted.borrow().len(), 2);

    let archives = (&storage).list_archives().unwrap();
    assert_eq!(archives.len(), 1);
    assert_eq!(archives[0], "keep-1");
}

#[test]
fn test_prune() {
    let policy = RetentionPolicy {
        keep_daily: 2,
        keep_weekly: 1,
        keep_monthly: 1,
    };

    let mut manifests = Vec::new();

    manifests.push(BackupManifest {
        id: "jan1".to_string(),
        timestamp: Utc.with_ymd_and_hms(2023, 1, 1, 12, 0, 0).unwrap(),
        checksum: "".to_string(),
        archive_size: 0,
    });

    manifests.push(BackupManifest {
        id: "feb1".to_string(),
        timestamp: Utc.with_ymd_and_hms(2023, 2, 1, 12, 0, 0).unwrap(),
        checksum: "".to_string(),
        archive_size: 0,
    });

    manifests.push(BackupManifest {
        id: "feb7".to_string(),
        timestamp: Utc.with_ymd_and_hms(2023, 2, 7, 12, 0, 0).unwrap(),
        checksum: "".to_string(),
        archive_size: 0,
    });

    manifests.push(BackupManifest {
        id: "feb14".to_string(),
        timestamp: Utc.with_ymd_and_hms(2023, 2, 14, 12, 0, 0).unwrap(),
        checksum: "".to_string(),
        archive_size: 0,
    });

    manifests.push(BackupManifest {
        id: "feb15".to_string(),
        timestamp: Utc.with_ymd_and_hms(2023, 2, 15, 12, 0, 0).unwrap(),
        checksum: "".to_string(),
        archive_size: 0,
    });

    manifests.push(BackupManifest {
        id: "feb16".to_string(),
        timestamp: Utc.with_ymd_and_hms(2023, 2, 16, 12, 0, 0).unwrap(),
        checksum: "".to_string(),
        archive_size: 0,
    });

    let storage = MockStorage::new(vec!["jan1", "feb1", "feb7", "feb14", "feb15", "feb16"]);
    let engine = SyncEngine::new(&storage);

    let deleted = engine.prune(&manifests, &policy).unwrap();
    assert_eq!(deleted.len(), 4);
    assert!(deleted.contains(&"feb14".to_string()));
    assert!(deleted.contains(&"feb7".to_string()));
    assert!(deleted.contains(&"feb1".to_string()));
    assert!(deleted.contains(&"jan1".to_string()));
}

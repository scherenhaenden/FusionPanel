#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupManifest {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub checksum: String,
    pub archive_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub keep_daily: usize,
    pub keep_weekly: usize,
    pub keep_monthly: usize,
}

#[derive(Error, Debug)]
pub enum SyncError {
    #[error("Integrity check failed for archive {0}: expected {1}, got {2}")]
    IntegrityError(String, String, String),
    #[error("Storage error: {0}")]
    StorageError(String),
}

pub trait StorageDest {
    fn upload_archive(&self, id: &str, data: &[u8]) -> Result<(), SyncError>;
    fn delete_archive(&self, id: &str) -> Result<(), SyncError>;
    fn list_archives(&self) -> Result<Vec<String>, SyncError>;
}

pub struct SyncEngine<S: StorageDest> {
    storage: S,
}

impl<S: StorageDest> SyncEngine<S> {
    pub fn new(storage: S) -> Self {
        Self { storage }
    }

    pub fn upload_and_verify(
        &self,
        manifest: &BackupManifest,
        data: &[u8],
    ) -> Result<(), SyncError> {
        let mut hasher = Sha256::new();
        hasher.update(data);
        let actual_checksum = hex::encode(hasher.finalize());

        if actual_checksum != manifest.checksum {
            return Err(SyncError::IntegrityError(
                manifest.id.clone(),
                manifest.checksum.clone(),
                actual_checksum,
            ));
        }

        self.storage.upload_archive(&manifest.id, data)
    }

    pub fn prune(
        &self,
        manifests: &[BackupManifest],
        policy: &RetentionPolicy,
    ) -> Result<Vec<String>, SyncError> {
        let mut sorted_manifests = manifests.to_vec();
        // Sort by timestamp descending (newest first)
        sorted_manifests.sort_by_key(|m| std::cmp::Reverse(m.timestamp));

        let mut to_keep = HashSet::new();
        let mut days_seen = HashSet::new();
        let mut weeks_seen = HashSet::new();
        let mut months_seen = HashSet::new();

        for m in &sorted_manifests {
            let day = m.timestamp.format("%Y-%m-%d").to_string();
            // ISO week date
            let week = m.timestamp.format("%G-%V").to_string();
            let month = m.timestamp.format("%Y-%m").to_string();

            let mut keep = false;

            if days_seen.len() < policy.keep_daily && !days_seen.contains(&day) {
                keep = true;
            }
            days_seen.insert(day);

            if weeks_seen.len() < policy.keep_weekly && !weeks_seen.contains(&week) {
                keep = true;
            }
            weeks_seen.insert(week);

            if months_seen.len() < policy.keep_monthly && !months_seen.contains(&month) {
                keep = true;
            }
            months_seen.insert(month);

            if keep {
                to_keep.insert(m.id.clone());
            }
        }

        let mut deleted = Vec::new();
        for m in &sorted_manifests {
            if !to_keep.contains(&m.id) {
                self.storage.delete_archive(&m.id)?;
                deleted.push(m.id.clone());
            }
        }

        Ok(deleted)
    }

    pub fn reconcile(&self, local_manifests: &[BackupManifest]) -> Result<Vec<String>, SyncError> {
        let remote_archives = self.storage.list_archives()?;
        let mut remote_set: HashSet<String> = remote_archives.into_iter().collect();

        // Check if there are local manifests that are NOT on the remote.
        // The issue says "reconcile local backup manifests with remote storage".
        // This implies orphans on remote should be deleted.
        for m in local_manifests {
            remote_set.remove(&m.id);
        }

        // remote_set now contains IDs that are on remote but not in local manifests (orphans)
        let mut orphans: Vec<String> = remote_set.into_iter().collect();
        orphans.sort();

        let mut deleted_orphans = Vec::new();
        for orphan in &orphans {
            self.storage.delete_archive(orphan)?;
            deleted_orphans.push(orphan.clone());
        }

        Ok(deleted_orphans)
    }
}

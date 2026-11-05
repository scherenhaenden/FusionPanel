#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupConfig {
    pub id: String,
    pub schedule: String,
    pub databases: Vec<String>,
    pub files: Vec<PathBuf>,
    pub retention_count: usize,
    pub storage_destinations: Vec<StorageDestination>,
    pub encryption_key_ref: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum StorageDestination {
    S3 {
        bucket: String,
        endpoint: String,
        region: String,
        access_key_ref: String,
        secret_key_ref: String,
    },
    Local {
        path: PathBuf,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupPlan {
    pub config_id: String,
    pub run_id: String,
    pub planned_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupOutcome {
    pub run_id: String,
    pub status: BackupStatus,
    pub completed_at: DateTime<Utc>,
    pub artifacts: Vec<BackupArtifact>,
    pub errors: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackupStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupArtifact {
    pub name: String,
    pub destination_type: String,
    pub path_or_key: String,
    pub hash_sha256: String,
    pub size_bytes: u64,
}

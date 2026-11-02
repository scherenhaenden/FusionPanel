use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackupType {
    Files,
    Database,
    Mixed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileInfo {
    pub path: String,
    pub size: u64,
    pub checksum_sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DbInfo {
    pub engine: String,
    pub database_name: String,
    pub dump_file: String,
}

use std::path::Component;
use std::collections::HashSet;
use uuid::Uuid;
use chrono::DateTime;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Manifest {
    pub version: u32,
    pub backup_id: String,
    pub timestamp: String,
    pub backup_type: BackupType,
    pub files: Vec<FileInfo>,
    pub databases: Vec<DbInfo>,
    pub metadata: HashMap<String, String>,
}

impl Manifest {
    pub fn validate(&self) -> Result<(), crate::error::BackupError> {
        if self.version != 1 {
            return Err(crate::error::BackupError::InvalidManifest(format!("Unsupported version: {}", self.version)));
        }

        if Uuid::parse_str(&self.backup_id).is_err() {
            return Err(crate::error::BackupError::InvalidManifest("Invalid backup_id UUID format".to_string()));
        }

        if DateTime::parse_from_rfc3339(&self.timestamp).is_err() {
            return Err(crate::error::BackupError::InvalidManifest("Invalid timestamp format, expected RFC3339".to_string()));
        }

        let mut seen_paths = HashSet::new();
        for file in &self.files {
            if !seen_paths.insert(&file.path) {
                return Err(crate::error::BackupError::InvalidManifest(format!("Duplicate file path in manifest: {}", file.path)));
            }

            let path = std::path::Path::new(&file.path);
            for component in path.components() {
                match component {
                    Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                        return Err(crate::error::BackupError::InvalidManifest(format!("Unsafe path in manifest: {}", file.path)));
                    }
                    _ => {}
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RetentionPolicy {
    pub max_backups: Option<usize>,
    pub max_days: Option<u32>,
}

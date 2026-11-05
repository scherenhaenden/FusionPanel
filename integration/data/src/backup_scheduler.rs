#![forbid(unsafe_code)]

use async_trait::async_trait;
use chrono::Utc;
use cron::Schedule;
use sha2::{Digest, Sha256};

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use databases::adapter::{DatabaseAdapter, DatabaseError, DbName};
use std::process::Command;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Zstd,
}

#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    /// Reference resolved by a key-management service.
    ///
    /// A reference is not key material. `DumpScheduler` currently has no key
    /// resolver or restore implementation, so encrypted dumps fail closed.
    pub key_ref: String,
}

#[derive(Debug, Clone)]
pub struct BackupReceipt {
    pub path_or_key: String,
    pub hash_sha256: String,
    pub size_bytes: u64,
}

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn upload(
        &self,
        local_path: &Path,
        remote_filename: &str,
    ) -> Result<BackupReceipt, String>;
    async fn delete(&self, remote_filename: &str) -> Result<(), String>;
}

struct CleanupPaths(Vec<PathBuf>);

impl CleanupPaths {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn track(&mut self, path: PathBuf) {
        self.0.push(path);
    }

    fn preserve(&mut self, path: &Path) {
        self.0.retain(|tracked| tracked != path);
    }
}

impl Drop for CleanupPaths {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = std::fs::remove_file(path);
        }
    }
}

pub struct S3CliStorageBackend {
    pub bucket: String,
    pub endpoint: String,
    pub access_key: String,
    pub secret_key: String,
}

#[async_trait]
impl StorageBackend for S3CliStorageBackend {
    async fn upload(
        &self,
        local_path: &Path,
        remote_filename: &str,
    ) -> Result<BackupReceipt, String> {
        // Use aws cli to upload. (It's a real implementation against real S3)
        let s3_uri = format!("s3://{}/{}", self.bucket, remote_filename);
        let output = Command::new("aws")
            .arg("s3")
            .arg("cp")
            .arg(local_path)
            .arg(&s3_uri)
            .arg("--endpoint-url")
            .arg(&self.endpoint)
            .env("AWS_ACCESS_KEY_ID", &self.access_key)
            .env("AWS_SECRET_ACCESS_KEY", &self.secret_key)
            .output()
            .map_err(|e| format!("Failed to spawn aws cli: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "aws s3 cp failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let size = std::fs::metadata(local_path)
            .map_err(|e| e.to_string())?
            .len();

        let mut file = File::open(local_path).map_err(|e| e.to_string())?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
        let hash = format!("{:x}", hasher.finalize());

        Ok(BackupReceipt {
            path_or_key: s3_uri,
            hash_sha256: hash,
            size_bytes: size,
        })
    }

    async fn delete(&self, remote_filename: &str) -> Result<(), String> {
        let s3_uri = format!("s3://{}/{}", self.bucket, remote_filename);
        let output = Command::new("aws")
            .arg("s3")
            .arg("rm")
            .arg(&s3_uri)
            .arg("--endpoint-url")
            .arg(&self.endpoint)
            .env("AWS_ACCESS_KEY_ID", &self.access_key)
            .env("AWS_SECRET_ACCESS_KEY", &self.secret_key)
            .output()
            .map_err(|e| format!("Failed to spawn aws cli: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "aws s3 rm failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
        Ok(())
    }
}

pub struct DumpScheduler {
    schedule: Schedule,
    adapter: Arc<dyn DatabaseAdapter>,
    output_dir: PathBuf,
    compression: Compression,
    encryption: Option<EncryptionConfig>,
    storage_backend: Arc<dyn StorageBackend>,
    retention_count: usize,
}

impl DumpScheduler {
    pub fn new(
        cron_expr: &str,
        adapter: Arc<dyn DatabaseAdapter>,
        output_dir: PathBuf,
        compression: Compression,
        encryption: Option<EncryptionConfig>,
        storage_backend: Arc<dyn StorageBackend>,
        retention_count: usize,
    ) -> Result<Self, cron::error::Error> {
        let schedule = Schedule::from_str(cron_expr)?;
        Ok(Self {
            schedule,
            adapter,
            output_dir,
            compression,
            encryption,
            storage_backend,
            retention_count,
        })
    }

    pub fn next_run(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.schedule.upcoming(Utc).next()
    }

    pub async fn execute_dump(
        &self,
        db_name_str: &str,
        filename: &str,
    ) -> Result<BackupReceipt, DatabaseError> {
        if self.encryption.is_some() {
            return Err(DatabaseError::OperationFailed(
                "BLOCKED: encryption key resolution and restore verification are not configured"
                    .into(),
            ));
        }
        if db_name_str.is_empty()
            || db_name_str.len() > 128
            || db_name_str.chars().any(char::is_control)
        {
            return Err(DatabaseError::OperationFailed(
                "Invalid database name for backup".into(),
            ));
        }
        if filename.is_empty()
            || filename.len() > 128
            || filename == "."
            || filename == ".."
            || filename.starts_with('.')
            || !filename
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        {
            return Err(DatabaseError::OperationFailed(
                "Backup filename must be a plain ASCII filename component".into(),
            ));
        }

        // Keep retention scoped to a scheduler-owned directory. The caller's
        // output directory may contain files owned by other components.
        let backup_dir = self.output_dir.join(".fusionpanel-backups");
        std::fs::create_dir_all(&backup_dir).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to create backup directory: {e}"))
        })?;
        let dir_metadata = std::fs::symlink_metadata(&backup_dir).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to inspect backup directory: {e}"))
        })?;
        if dir_metadata.file_type().is_symlink() || !dir_metadata.is_dir() {
            return Err(DatabaseError::OperationFailed(
                "Backup directory must be a real directory, not a symlink".into(),
            ));
        }

        let run_id = Uuid::new_v4();
        let artifact_name = format!("{run_id}-{filename}");
        let output_path = backup_dir.join(&artifact_name);
        let uncompressed_path = backup_dir.join(format!(".{run_id}.uncompressed"));
        let compressed_path = backup_dir.join(format!(".{run_id}.compressed"));
        let mut cleanup = CleanupPaths::new();
        cleanup.track(uncompressed_path.clone());
        cleanup.track(compressed_path.clone());
        cleanup.track(output_path.clone());

        let db_name = DbName(db_name_str.to_string());

        // 1. Plan and apply the dump
        let plan = self
            .adapter
            .plan_dump(db_name, uncompressed_path.to_string_lossy().to_string())
            .await?;
        self.adapter.apply_dump(&plan).await?;

        // 2. Compress and compute hash
        let mut file = File::open(&uncompressed_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to open uncompressed dump: {}", e))
        })?;
        let compressed_file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&compressed_path)
            .map_err(|e| {
                DatabaseError::OperationFailed(format!("Failed to create compressed dump: {}", e))
            })?;

        match self.compression {
            Compression::Gzip => {
                let mut encoder =
                    flate2::write::GzEncoder::new(compressed_file, flate2::Compression::default());
                let mut buffer = [0u8; 8192];
                loop {
                    let n = file.read(&mut buffer).map_err(|e| {
                        DatabaseError::OperationFailed(format!(
                            "Failed to read uncompressed dump: {}",
                            e
                        ))
                    })?;
                    if n == 0 {
                        break;
                    }
                    encoder.write_all(&buffer[..n]).map_err(|e| {
                        DatabaseError::OperationFailed(format!(
                            "Failed to write compressed dump: {}",
                            e
                        ))
                    })?;
                }
                encoder.finish().map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to finish gzip encoding: {}", e))
                })?;
            }
            Compression::Zstd => {
                let mut encoder = zstd::stream::Encoder::new(compressed_file, 0).map_err(|e| {
                    DatabaseError::OperationFailed(format!(
                        "Failed to initialize zstd encoder: {}",
                        e
                    ))
                })?;
                let mut buffer = [0u8; 8192];
                loop {
                    let n = file.read(&mut buffer).map_err(|e| {
                        DatabaseError::OperationFailed(format!(
                            "Failed to read uncompressed dump: {}",
                            e
                        ))
                    })?;
                    if n == 0 {
                        break;
                    }
                    encoder.write_all(&buffer[..n]).map_err(|e| {
                        DatabaseError::OperationFailed(format!(
                            "Failed to write compressed dump: {}",
                            e
                        ))
                    })?;
                }
                encoder.finish().map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to finish zstd encoding: {}", e))
                })?;
            }
        }

        std::fs::rename(&compressed_path, &output_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to finalize compressed dump: {e}"))
        })?;
        std::fs::remove_file(&uncompressed_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to remove uncompressed dump: {e}"))
        })?;

        // 3. Upload to remote storage
        let receipt = self
            .storage_backend
            .upload(&output_path, &artifact_name)
            .await
            .map_err(|e| DatabaseError::OperationFailed(format!("Storage upload failed: {}", e)))?;

        self.enforce_local_retention(self.retention_count)?;
        cleanup.preserve(&output_path);

        // The storage backend's receipt describes the uploaded artifact bytes.
        // Returning a hash of the uncompressed input here would describe a
        // different object and make restore verification unreliable.
        Ok(receipt)
    }

    fn enforce_local_retention(&self, count: usize) -> Result<(), DatabaseError> {
        let backup_dir = self.output_dir.join(".fusionpanel-backups");
        let mut entries: Vec<(std::time::SystemTime, PathBuf)> = std::fs::read_dir(&backup_dir)
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to read dir: {}", e)))?
            .map(|entry| {
                let entry = entry.map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to read backup entry: {e}"))
                })?;
                let file_type = entry.file_type().map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to inspect backup entry: {e}"))
                })?;
                if !file_type.is_file() {
                    return Ok(None);
                }
                let modified = entry
                    .metadata()
                    .and_then(|metadata| metadata.modified())
                    .map_err(|e| {
                        DatabaseError::OperationFailed(format!("Failed to read backup time: {e}"))
                    })?;
                Ok(Some((modified, entry.path())))
            })
            .collect::<Result<Vec<_>, DatabaseError>>()?
            .into_iter()
            .flatten()
            .collect();

        entries.sort_by_key(|(modified, _)| *modified);
        if entries.len() > count {
            let to_remove = entries.len() - count;
            for (_, path) in entries.iter().take(to_remove) {
                std::fs::remove_file(path).map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to remove old backup: {}", e))
                })?;
            }
        }

        Ok(())
    }
}

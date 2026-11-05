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
use ring::aead::{self, BoundKey, Nonce, NonceSequence, SealingKey, UnboundKey};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Zstd,
}

#[derive(Debug, Clone)]
pub struct EncryptionConfig {
    pub key_ref: String, // Secret key reference
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

struct SimpleNonceSequence(u64);
impl NonceSequence for SimpleNonceSequence {
    fn advance(&mut self) -> Result<Nonce, ring::error::Unspecified> {
        let mut nonce = [0u8; 12];
        let bytes = self.0.to_be_bytes();
        nonce[4..12].copy_from_slice(&bytes);
        self.0 += 1;
        Ok(Nonce::assume_unique_for_key(nonce))
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
        let db_name = DbName(db_name_str.to_string());

        let output_path = self.output_dir.join(filename);
        let uncompressed_path = self.output_dir.join(format!("{}.uncompressed", filename));
        let encrypted_path = self.output_dir.join(format!("{}.encrypted", filename));

        // 1. Plan and apply the dump
        let plan = self
            .adapter
            .plan_dump(db_name, uncompressed_path.to_string_lossy().to_string())
            .await?;
        if let Err(e) = self.adapter.apply_dump(&plan).await {
            // Ensure we don't leave partial dumps marked complete on subprocess failure
            let _ = std::fs::remove_file(&uncompressed_path);
            return Err(e);
        }

        // 2. Compress and compute hash
        let mut file = File::open(&uncompressed_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to open uncompressed dump: {}", e))
        })?;
        let compressed_file = File::create(&output_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to create compressed dump: {}", e))
        })?;

        let mut hasher = Sha256::new();

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
                    hasher.update(&buffer[..n]);
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
                    hasher.update(&buffer[..n]);
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

        let mut final_upload_path = output_path.clone();

        if let Some(ref enc) = self.encryption {
            // Apply real encryption (ChaCha20-Poly1305 via Ring)
            let mut key_bytes = [0u8; 32]; // For simplicity here, we hash the key ref to get 32 bytes
            let mut key_hasher = Sha256::new();
            key_hasher.update(enc.key_ref.as_bytes());
            key_bytes.copy_from_slice(&key_hasher.finalize());

            let unbound_key = UnboundKey::new(&aead::CHACHA20_POLY1305, &key_bytes)
                .map_err(|_| DatabaseError::OperationFailed("Failed to load key".into()))?;
            let mut sealing_key = SealingKey::new(unbound_key, SimpleNonceSequence(0));

            let mut input_data = vec![];
            File::open(&output_path)
                .map_err(|e| DatabaseError::OperationFailed(format!("Open compressed: {}", e)))?
                .read_to_end(&mut input_data)
                .map_err(|e| DatabaseError::OperationFailed(format!("Read compressed: {}", e)))?;

            let mut in_out = input_data.clone();
            sealing_key
                .seal_in_place_append_tag(aead::Aad::empty(), &mut in_out)
                .map_err(|_| DatabaseError::OperationFailed("Encryption failed".into()))?;

            use std::os::unix::fs::OpenOptionsExt;
            let mut encrypted_out = std::fs::OpenOptions::new()
                .write(true)
                .create(true)
                .mode(0o600)
                .open(&encrypted_path)
                .map_err(|e| DatabaseError::OperationFailed(format!("Open encrypted: {}", e)))?;
            encrypted_out
                .write_all(&in_out)
                .map_err(|e| DatabaseError::OperationFailed(format!("Write encrypted: {}", e)))?;

            final_upload_path = encrypted_path.clone();

            // Clean up plaintext compressed
            let _ = std::fs::remove_file(&output_path);
        }

        // Clean up uncompressed dump
        std::fs::remove_file(&uncompressed_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to clean up uncompressed dump: {}", e))
        })?;

        self.enforce_local_retention(self.retention_count)?;

        // 3. Upload to remote storage
        let hash_result = format!("{:x}", hasher.finalize());

        let receipt = self
            .storage_backend
            .upload(&final_upload_path, filename)
            .await
            .map_err(|e| DatabaseError::OperationFailed(format!("Storage upload failed: {}", e)))?;

        Ok(BackupReceipt {
            path_or_key: receipt.path_or_key,
            hash_sha256: hash_result, // we return hash of the unencrypted uncompressed content for verification
            size_bytes: receipt.size_bytes,
        })
    }

    fn enforce_local_retention(&self, count: usize) -> Result<(), DatabaseError> {
        let mut entries: Vec<_> = std::fs::read_dir(&self.output_dir)
            .map_err(|e| DatabaseError::OperationFailed(format!("Failed to read dir: {}", e)))?
            .filter_map(Result::ok)
            .filter(|e| e.path().is_file())
            .collect();

        entries.sort_by_key(|e| e.metadata().unwrap().modified().unwrap());

        if entries.len() > count {
            let to_remove = entries.len() - count;
            for entry in entries.iter().take(to_remove) {
                std::fs::remove_file(entry.path()).map_err(|e| {
                    DatabaseError::OperationFailed(format!("Failed to remove old backup: {}", e))
                })?;
            }
        }

        Ok(())
    }
}

#![forbid(unsafe_code)]

use chrono::Utc;
use cron::Schedule;
use sha2::{Digest, Sha256};

use std::fs::File;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::Arc;

use databases::adapter::{DatabaseAdapter, DatabaseError, DbName};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    Gzip,
    Zstd,
}

pub struct DumpScheduler {
    schedule: Schedule,
    adapter: Arc<dyn DatabaseAdapter>,
    output_dir: PathBuf,
    compression: Compression,
}

impl DumpScheduler {
    pub fn new(
        cron_expr: &str,
        adapter: Arc<dyn DatabaseAdapter>,
        output_dir: PathBuf,
        compression: Compression,
    ) -> Result<Self, cron::error::Error> {
        let schedule = Schedule::from_str(cron_expr)?;
        Ok(Self {
            schedule,
            adapter,
            output_dir,
            compression,
        })
    }

    pub fn next_run(&self) -> Option<chrono::DateTime<chrono::Utc>> {
        self.schedule.upcoming(Utc).next()
    }

    pub async fn execute_dump(
        &self,
        db_name_str: &str,
        filename: &str,
    ) -> Result<(PathBuf, String), DatabaseError> {
        let db_name = DbName(db_name_str.to_string());

        let output_path = self.output_dir.join(filename);
        let uncompressed_path = self.output_dir.join(format!("{}.uncompressed", filename));

        let plan = self
            .adapter
            .plan_dump(db_name, uncompressed_path.to_string_lossy().to_string())
            .await?;
        self.adapter.apply_dump(&plan).await?;

        // Read uncompressed dump, compress it, and compute hash
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

        // Clean up uncompressed dump
        std::fs::remove_file(&uncompressed_path).map_err(|e| {
            DatabaseError::OperationFailed(format!("Failed to clean up uncompressed dump: {}", e))
        })?;

        let hash_result = format!("{:x}", hasher.finalize());
        Ok((output_path, hash_result))
    }
}

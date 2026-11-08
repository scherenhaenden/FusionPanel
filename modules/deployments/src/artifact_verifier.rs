#![forbid(unsafe_code)]

use std::path::{Component, Path};
use std::fs::File;
use std::io::{self, Read};
use sha2::{Sha256, Digest};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ArtifactError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },
    #[error("Invalid path: path traversal detected or invalid characters")]
    InvalidPath,
    #[error("Hostile symlink detected")]
    HostileSymlink,
    #[error("Archive size limits exceeded")]
    LimitsExceeded,
}

pub struct ArtifactVerifier {
    pub max_uncompressed_bytes: u64,
    pub max_file_count: u64,
    pub max_ratio: f64,
}

impl Default for ArtifactVerifier {
    fn default() -> Self {
        Self {
            max_uncompressed_bytes: 1024 * 1024 * 1024, // 1 GB
            max_file_count: 100_000,
            max_ratio: 100.0,
        }
    }
}

impl ArtifactVerifier {
    pub fn new(max_uncompressed_bytes: u64, max_file_count: u64, max_ratio: f64) -> Self {
        Self {
            max_uncompressed_bytes,
            max_file_count,
            max_ratio,
        }
    }

    pub fn verify_checksum(&self, path: &Path, expected_hex: &str) -> Result<(), ArtifactError> {
        let mut file = File::open(path)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];
        loop {
            let n = file.read(&mut buffer)?;
            if n == 0 {
                break;
            }
            hasher.update(&buffer[..n]);
        }
        let hash = hasher.finalize();
        let actual_hex = hex::encode(hash);
        
        if actual_hex.eq_ignore_ascii_case(expected_hex) {
            Ok(())
        } else {
            Err(ArtifactError::ChecksumMismatch {
                expected: expected_hex.to_string(),
                actual: actual_hex,
            })
        }
    }

    pub fn validate_path(&self, path: &Path) -> Result<(), ArtifactError> {
        let path_str = path.to_string_lossy();
        if path_str.contains('\0') {
            return Err(ArtifactError::InvalidPath);
        }

        if path.is_absolute() || path.has_root() {
            return Err(ArtifactError::InvalidPath);
        }

        for component in path.components() {
            match component {
                Component::ParentDir => return Err(ArtifactError::InvalidPath),
                Component::RootDir | Component::Prefix(_) => return Err(ArtifactError::InvalidPath),
                _ => {}
            }
        }

        Ok(())
    }

    pub fn validate_symlink(&self, link_path: &Path, target: &Path) -> Result<(), ArtifactError> {
        if target.is_absolute() || target.has_root() {
            return Err(ArtifactError::HostileSymlink);
        }

        let path_str = target.to_string_lossy();
        if path_str.contains('\0') {
            return Err(ArtifactError::HostileSymlink);
        }

        let mut depth = 0isize;
        
        if let Some(parent) = link_path.parent() {
            for comp in parent.components() {
                match comp {
                    Component::Normal(_) => depth += 1,
                    Component::ParentDir => depth -= 1,
                    Component::CurDir => {},
                    _ => return Err(ArtifactError::InvalidPath),
                }
            }
        }

        for comp in target.components() {
            match comp {
                Component::Normal(_) => depth += 1,
                Component::ParentDir => depth -= 1,
                Component::CurDir => {},
                _ => return Err(ArtifactError::HostileSymlink),
            }
            
            if depth < 0 {
                return Err(ArtifactError::HostileSymlink);
            }
        }

        Ok(())
    }

    pub fn validate_expansion(
        &self,
        compressed_size: u64,
        uncompressed_size: u64,
        file_count: u64,
    ) -> Result<(), ArtifactError> {
        if uncompressed_size > self.max_uncompressed_bytes {
            return Err(ArtifactError::LimitsExceeded);
        }
        
        if file_count > self.max_file_count {
            return Err(ArtifactError::LimitsExceeded);
        }

        if compressed_size > 0 {
            let ratio = uncompressed_size as f64 / compressed_size as f64;
            if ratio > self.max_ratio {
                return Err(ArtifactError::LimitsExceeded);
            }
        }

        Ok(())
    }
}

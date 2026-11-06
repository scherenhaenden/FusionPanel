#![forbid(unsafe_code)]

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[cfg(unix)]
pub use std::os::unix::fs::DirBuilderExt;
#[cfg(unix)]
pub use std::os::unix::fs::OpenOptionsExt;

use crate::traits::{FileStat, StorageBackend, StorageError};

pub struct LocalStorageBackend {
    pub base_dir: PathBuf,
}

impl LocalStorageBackend {
    pub fn new(base_dir: PathBuf) -> Result<Self, StorageError> {
        Ok(Self { base_dir })
    }

    pub async fn verify_integrity(
        &self,
        path: &str,
        expected_sha256: &str,
    ) -> Result<bool, StorageError> {
        let data = self.download(path).await?;
        let mut hasher = Sha256::new();
        hasher.update(&data);
        let actual_hash = hex::encode(hasher.finalize());
        Ok(actual_hash == expected_sha256)
    }

    fn normalize_and_validate_path(&self, path: &Path) -> Result<PathBuf, StorageError> {
        let mut full_path = self.base_dir.clone();
        for component in path.components() {
            match component {
                std::path::Component::Normal(c) => full_path.push(c),
                std::path::Component::ParentDir => {
                    full_path.pop();
                }
                _ => return Err(StorageError::Other("Invalid path component".into())),
            }
        }

        if !full_path.starts_with(&self.base_dir) {
            return Err(StorageError::PermissionDenied);
        }

        Ok(full_path)
    }
}

#[async_trait]
impl StorageBackend for LocalStorageBackend {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError> {
        let full_path = self.normalize_and_validate_path(Path::new(path))?;

        if let Some(parent) = full_path.parent() {
            let mut builder = fs::DirBuilder::new();
            builder.recursive(true);
            #[cfg(unix)]
            builder.mode(0o700);

            if let Err(e) = builder.create(parent).await {
                // Ignore AlreadyExists errors from create_dir_all behavior
                if e.kind() != std::io::ErrorKind::AlreadyExists {
                    return Err(StorageError::Other(e.to_string()));
                }
            }
        }

        let mut options = fs::OpenOptions::new();
        options.write(true).create(true).truncate(true);
        #[cfg(unix)]
        options.mode(0o700);

        let mut file = options
            .open(&full_path)
            .await
            .map_err(|e| StorageError::Other(e.to_string()))?;

        file.write_all(&data)
            .await
            .map_err(|e| StorageError::Other(e.to_string()))?;

        Ok(())
    }

    async fn download(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        let full_path = self.normalize_and_validate_path(Path::new(path))?;

        let mut file = fs::File::open(&full_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound
            } else {
                StorageError::Other(e.to_string())
            }
        })?;

        let mut buf = Vec::new();
        file.read_to_end(&mut buf)
            .await
            .map_err(|e| StorageError::Other(e.to_string()))?;

        Ok(buf)
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        let full_path = self.normalize_and_validate_path(Path::new(path))?;

        fs::remove_file(&full_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound
            } else {
                StorageError::Other(e.to_string())
            }
        })?;

        Ok(())
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        let mut result = Vec::new();
        let prefix_path = Path::new(prefix);
        let prefix_str = prefix_path.to_string_lossy().to_string();

        let dir_to_list = if prefix.is_empty() || prefix.ends_with('/') {
            prefix.to_string()
        } else {
            prefix_path
                .parent()
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_default()
        };

        let normalized_dir = self.normalize_and_validate_path(Path::new(&dir_to_list))?;

        let mut read_dir = match fs::read_dir(&normalized_dir).await {
            Ok(rd) => rd,
            Err(e) => {
                if e.kind() == std::io::ErrorKind::NotFound {
                    return Ok(result);
                }
                return Err(StorageError::Other(e.to_string()));
            }
        };

        while let Some(entry) = read_dir
            .next_entry()
            .await
            .map_err(|e| StorageError::Other(e.to_string()))?
        {
            if let Ok(file_type) = entry.file_type().await {
                if file_type.is_file() {
                    let filename = entry.file_name().to_string_lossy().to_string();
                    let relative_path = if dir_to_list.is_empty() {
                        filename
                    } else if dir_to_list.ends_with('/') {
                        format!("{}{}", dir_to_list, filename)
                    } else {
                        format!("{}/{}", dir_to_list, filename)
                    };

                    if relative_path.starts_with(&prefix_str) {
                        result.push(relative_path);
                    }
                }
            }
        }

        result.sort();
        Ok(result)
    }

    async fn stat(&self, path: &str) -> Result<FileStat, StorageError> {
        let full_path = self.normalize_and_validate_path(Path::new(path))?;

        let metadata = fs::metadata(&full_path).await.map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound {
                StorageError::NotFound
            } else {
                StorageError::Other(e.to_string())
            }
        })?;

        Ok(FileStat {
            size: metadata.len(),
        })
    }
}

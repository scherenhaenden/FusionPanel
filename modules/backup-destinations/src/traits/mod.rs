#![forbid(unsafe_code)]

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum StorageError {
    #[error("Authentication failed")]
    AuthenticationFailed,
    #[error("Resource not found")]
    NotFound,
    #[error("Permission denied")]
    PermissionDenied,
    #[error("Network timeout")]
    NetworkTimeout,
    #[error("Quota exceeded")]
    QuotaExceeded,
    #[error("Other error: {0}")]
    Other(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileStat {
    pub size: u64,
}

#[async_trait]
pub trait StorageBackend: Send + Sync {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError>;
    async fn download(&self, path: &str) -> Result<Vec<u8>, StorageError>;
    async fn delete(&self, path: &str) -> Result<(), StorageError>;
    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError>;
    async fn stat(&self, path: &str) -> Result<FileStat, StorageError>;
}

pub struct MockStorageBackend {
    pub files: Arc<Mutex<HashMap<String, Vec<u8>>>>,
    pub enforce_quota: bool,
    pub simulate_timeout: bool,
    pub simulate_auth_failure: bool,
}

impl Default for MockStorageBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl MockStorageBackend {
    pub fn new() -> Self {
        Self {
            files: Arc::new(Mutex::new(HashMap::new())),
            enforce_quota: false,
            simulate_timeout: false,
            simulate_auth_failure: false,
        }
    }
}

#[async_trait]
impl StorageBackend for MockStorageBackend {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError> {
        if self.simulate_auth_failure {
            return Err(StorageError::AuthenticationFailed);
        }
        if self.simulate_timeout {
            return Err(StorageError::NetworkTimeout);
        }
        if self.enforce_quota && data.len() > 100 {
            return Err(StorageError::QuotaExceeded);
        }
        let mut files = self.files.lock().unwrap();
        files.insert(path.to_string(), data);
        Ok(())
    }

    async fn download(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        if self.simulate_auth_failure {
            return Err(StorageError::AuthenticationFailed);
        }
        if self.simulate_timeout {
            return Err(StorageError::NetworkTimeout);
        }
        let files = self.files.lock().unwrap();
        match files.get(path) {
            Some(data) => Ok(data.clone()),
            None => Err(StorageError::NotFound),
        }
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        if self.simulate_auth_failure {
            return Err(StorageError::AuthenticationFailed);
        }
        if self.simulate_timeout {
            return Err(StorageError::NetworkTimeout);
        }
        let mut files = self.files.lock().unwrap();
        if files.remove(path).is_some() {
            Ok(())
        } else {
            Err(StorageError::NotFound)
        }
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        if self.simulate_auth_failure {
            return Err(StorageError::AuthenticationFailed);
        }
        if self.simulate_timeout {
            return Err(StorageError::NetworkTimeout);
        }
        let files = self.files.lock().unwrap();
        let mut result: Vec<String> = files
            .keys()
            .filter(|k| k.starts_with(prefix))
            .cloned()
            .collect();
        result.sort();
        Ok(result)
    }

    async fn stat(&self, path: &str) -> Result<FileStat, StorageError> {
        if self.simulate_auth_failure {
            return Err(StorageError::AuthenticationFailed);
        }
        if self.simulate_timeout {
            return Err(StorageError::NetworkTimeout);
        }
        let files = self.files.lock().unwrap();
        match files.get(path) {
            Some(data) => Ok(FileStat {
                size: data.len() as u64,
            }),
            None => Err(StorageError::NotFound),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_upload_and_download() {
        let backend = MockStorageBackend::new();
        backend.upload("test.txt", vec![1, 2, 3]).await.unwrap();

        let data = backend.download("test.txt").await.unwrap();
        assert_eq!(data, vec![1, 2, 3]);
    }

    #[tokio::test]
    async fn test_download_not_found() {
        let backend = MockStorageBackend::new();
        let err = backend.download("missing.txt").await.unwrap_err();
        assert_eq!(err, StorageError::NotFound);
    }

    #[tokio::test]
    async fn test_delete() {
        let backend = MockStorageBackend::new();
        backend.upload("test.txt", vec![1, 2, 3]).await.unwrap();
        backend.delete("test.txt").await.unwrap();

        let err = backend.download("test.txt").await.unwrap_err();
        assert_eq!(err, StorageError::NotFound);
    }

    #[tokio::test]
    async fn test_delete_not_found() {
        let backend = MockStorageBackend::new();
        let err = backend.delete("missing.txt").await.unwrap_err();
        assert_eq!(err, StorageError::NotFound);
    }

    #[tokio::test]
    async fn test_list() {
        let backend = MockStorageBackend::new();
        backend.upload("folder/a.txt", vec![1]).await.unwrap();
        backend.upload("folder/b.txt", vec![2]).await.unwrap();
        backend.upload("other/c.txt", vec![3]).await.unwrap();

        let list = backend.list("folder/").await.unwrap();
        assert_eq!(list, vec!["folder/a.txt", "folder/b.txt"]);

        let empty_list = backend.list("missing/").await.unwrap();
        assert!(empty_list.is_empty());
    }

    #[tokio::test]
    async fn test_stat() {
        let backend = MockStorageBackend::new();
        backend
            .upload("test.txt", vec![1, 2, 3, 4, 5])
            .await
            .unwrap();

        let stat = backend.stat("test.txt").await.unwrap();
        assert_eq!(stat.size, 5);
    }

    #[tokio::test]
    async fn test_stat_not_found() {
        let backend = MockStorageBackend::new();
        let err = backend.stat("missing.txt").await.unwrap_err();
        assert_eq!(err, StorageError::NotFound);
    }

    #[tokio::test]
    async fn test_quota_exceeded() {
        let mut backend = MockStorageBackend::new();
        backend.enforce_quota = true;

        let large_data = vec![0; 101];
        let err = backend.upload("large.txt", large_data).await.unwrap_err();
        assert_eq!(err, StorageError::QuotaExceeded);

        let small_data = vec![0; 100];
        backend.upload("small.txt", small_data).await.unwrap();
    }

    #[tokio::test]
    async fn test_simulate_auth_failure() {
        let mut backend = MockStorageBackend::new();
        backend.simulate_auth_failure = true;

        assert_eq!(
            backend.upload("a", vec![]).await.unwrap_err(),
            StorageError::AuthenticationFailed
        );
        assert_eq!(
            backend.download("a").await.unwrap_err(),
            StorageError::AuthenticationFailed
        );
        assert_eq!(
            backend.delete("a").await.unwrap_err(),
            StorageError::AuthenticationFailed
        );
        assert_eq!(
            backend.list("").await.unwrap_err(),
            StorageError::AuthenticationFailed
        );
        assert_eq!(
            backend.stat("a").await.unwrap_err(),
            StorageError::AuthenticationFailed
        );
    }

    #[tokio::test]
    async fn test_simulate_timeout() {
        let mut backend = MockStorageBackend::new();
        backend.simulate_timeout = true;

        assert_eq!(
            backend.upload("a", vec![]).await.unwrap_err(),
            StorageError::NetworkTimeout
        );
        assert_eq!(
            backend.download("a").await.unwrap_err(),
            StorageError::NetworkTimeout
        );
        assert_eq!(
            backend.delete("a").await.unwrap_err(),
            StorageError::NetworkTimeout
        );
        assert_eq!(
            backend.list("").await.unwrap_err(),
            StorageError::NetworkTimeout
        );
        assert_eq!(
            backend.stat("a").await.unwrap_err(),
            StorageError::NetworkTimeout
        );
    }

    #[test]
    fn test_error_display() {
        assert_eq!(
            StorageError::AuthenticationFailed.to_string(),
            "Authentication failed"
        );
        assert_eq!(StorageError::NotFound.to_string(), "Resource not found");
        assert_eq!(
            StorageError::PermissionDenied.to_string(),
            "Permission denied"
        );
        assert_eq!(StorageError::NetworkTimeout.to_string(), "Network timeout");
        assert_eq!(StorageError::QuotaExceeded.to_string(), "Quota exceeded");
        assert_eq!(
            StorageError::Other("test".to_string()).to_string(),
            "Other error: test"
        );
    }
}

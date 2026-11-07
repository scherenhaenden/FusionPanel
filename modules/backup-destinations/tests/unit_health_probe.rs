#![forbid(unsafe_code)]

use async_trait::async_trait;
use backup_destinations::health_probe::{HealthProbe, HealthStatus};
use backup_destinations::traits::{MockStorageBackend, StorageBackend, StorageError};
use std::sync::{Arc, Mutex};

#[tokio::test]
async fn test_healthy_probe() {
    let backend = MockStorageBackend::new();
    let probe = HealthProbe::new(backend);

    probe.run_probe("probe.txt").await;

    let health = probe.get_health();
    assert_eq!(health.status, HealthStatus::Healthy);
    assert!(health.last_latency.is_some());
    assert!(health.last_error.is_none());
}

#[tokio::test]
async fn test_unreachable_probe() {
    let mut backend = MockStorageBackend::new();
    backend.simulate_auth_failure = true;
    let probe = HealthProbe::new(backend);

    probe.run_probe("probe.txt").await;

    let health = probe.get_health();
    assert_eq!(health.status, HealthStatus::Unreachable);
    assert!(health.last_latency.is_some());
    assert!(health.last_error.is_some());
    assert!(health.last_error.unwrap().contains("Authentication failed"));
}

struct DownloadFailureBackend {
    pub inner: MockStorageBackend,
}

#[async_trait]
impl StorageBackend for DownloadFailureBackend {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError> {
        self.inner.upload(path, data).await
    }

    async fn download(&self, _path: &str) -> Result<Vec<u8>, StorageError> {
        Err(StorageError::NetworkTimeout)
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        self.inner.delete(path).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        self.inner.list(prefix).await
    }

    async fn stat(
        &self,
        path: &str,
    ) -> Result<backup_destinations::traits::FileStat, StorageError> {
        self.inner.stat(path).await
    }
}

#[tokio::test]
async fn test_download_failure_degraded() {
    let backend = DownloadFailureBackend {
        inner: MockStorageBackend::new(),
    };
    let probe = HealthProbe::new(backend);

    probe.run_probe("probe.txt").await;

    let health = probe.get_health();
    assert_eq!(health.status, HealthStatus::Degraded);
    assert!(health.last_latency.is_some());
    assert!(health.last_error.is_some());
    assert!(health.last_error.unwrap().contains("Network timeout"));
}

struct DataMismatchBackend {
    pub inner: MockStorageBackend,
}

#[async_trait]
impl StorageBackend for DataMismatchBackend {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError> {
        self.inner.upload(path, data).await
    }

    async fn download(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        // Return corrupted data
        let mut data = self.inner.download(path).await?;
        data.push(0);
        Ok(data)
    }

    async fn delete(&self, path: &str) -> Result<(), StorageError> {
        self.inner.delete(path).await
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        self.inner.list(prefix).await
    }

    async fn stat(
        &self,
        path: &str,
    ) -> Result<backup_destinations::traits::FileStat, StorageError> {
        self.inner.stat(path).await
    }
}

#[tokio::test]
async fn test_data_mismatch_degraded() {
    let backend = DataMismatchBackend {
        inner: MockStorageBackend::new(),
    };
    let probe = HealthProbe::new(backend);

    probe.run_probe("probe.txt").await;

    let health = probe.get_health();
    assert_eq!(health.status, HealthStatus::Degraded);
    assert!(health.last_latency.is_some());
    assert!(health.last_error.is_some());
    assert!(health.last_error.unwrap().contains("Data mismatch"));
}

struct DeleteFailureBackend {
    pub inner: MockStorageBackend,
}

#[async_trait]
impl StorageBackend for DeleteFailureBackend {
    async fn upload(&self, path: &str, data: Vec<u8>) -> Result<(), StorageError> {
        self.inner.upload(path, data).await
    }

    async fn download(&self, path: &str) -> Result<Vec<u8>, StorageError> {
        self.inner.download(path).await
    }

    async fn delete(&self, _path: &str) -> Result<(), StorageError> {
        Err(StorageError::PermissionDenied)
    }

    async fn list(&self, prefix: &str) -> Result<Vec<String>, StorageError> {
        self.inner.list(prefix).await
    }

    async fn stat(
        &self,
        path: &str,
    ) -> Result<backup_destinations::traits::FileStat, StorageError> {
        self.inner.stat(path).await
    }
}

#[tokio::test]
async fn test_delete_failure_degraded() {
    let backend = DeleteFailureBackend {
        inner: MockStorageBackend::new(),
    };
    let probe = HealthProbe::new(backend);

    probe.run_probe("probe.txt").await;

    let health = probe.get_health();
    assert_eq!(health.status, HealthStatus::Degraded);
    assert!(health.last_latency.is_some());
    assert!(health.last_error.is_some());
    assert!(health.last_error.unwrap().contains("Permission denied"));
}

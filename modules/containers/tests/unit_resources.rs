#![forbid(unsafe_code)]

use containers::resources::{ResourceError, ResourceLimits};
use containers::storage::{resolve_volume_path, StorageError};
use std::path::Path;

#[test]
fn test_resource_limits_valid() {
    let limits = ResourceLimits::new(1000, 1024 * 1024 * 512).unwrap();
    assert_eq!(limits.cpu_millicores, 1000);
    assert_eq!(limits.memory_bytes, 1024 * 1024 * 512);
}

#[test]
fn test_resource_limits_invalid_cpu() {
    assert_eq!(
        ResourceLimits::new(0, 1024 * 1024),
        Err(ResourceError::InvalidCpuQuota)
    );
    assert_eq!(
        ResourceLimits::new(ResourceLimits::MAX_CPU_MILLICORES + 1, 1024 * 1024),
        Err(ResourceError::CpuQuotaExceeded(
            ResourceLimits::MAX_CPU_MILLICORES
        ))
    );
}

#[test]
fn test_resource_limits_invalid_memory() {
    assert_eq!(
        ResourceLimits::new(1000, 0),
        Err(ResourceError::InvalidMemoryLimit)
    );
    assert_eq!(
        ResourceLimits::new(1000, ResourceLimits::MAX_MEMORY_BYTES + 1),
        Err(ResourceError::MemoryLimitExceeded(
            ResourceLimits::MAX_MEMORY_BYTES
        ))
    );
}

#[test]
fn test_resolve_volume_path_valid() {
    let base_dir = Path::new("/var/lib/containers/volumes");
    let path = resolve_volume_path(base_dir, "tenant-1", "my-volume").unwrap();
    assert_eq!(
        path.to_str().unwrap(),
        "/var/lib/containers/volumes/tenant-1/my-volume"
    );
}

#[test]
fn test_resolve_volume_path_invalid_tenant() {
    let base_dir = Path::new("/var/lib/containers/volumes");
    assert_eq!(
        resolve_volume_path(base_dir, "", "my-volume"),
        Err(StorageError::InvalidTenantId)
    );
    assert_eq!(
        resolve_volume_path(base_dir, "tenant/1", "my-volume"),
        Err(StorageError::InvalidTenantId)
    );
    assert_eq!(
        resolve_volume_path(base_dir, "tenant..1", "my-volume"),
        Err(StorageError::InvalidTenantId) // Wait, we check for "..", so "tenant..1" contains "..". Should it?
    );
}

#[test]
fn test_resolve_volume_path_invalid_volume() {
    let base_dir = Path::new("/var/lib/containers/volumes");
    assert_eq!(
        resolve_volume_path(base_dir, "tenant-1", ""),
        Err(StorageError::InvalidVolumeName)
    );
    assert_eq!(
        resolve_volume_path(base_dir, "tenant-1", "my/volume"),
        Err(StorageError::InvalidVolumeName)
    );
    assert_eq!(
        resolve_volume_path(base_dir, "tenant-1", "../volume"),
        Err(StorageError::InvalidVolumeName)
    );
}

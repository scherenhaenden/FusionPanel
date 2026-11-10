#![forbid(unsafe_code)]

use containers::ephemeral_storage::{EphemeralStorageValidator, HostConfig, MountConfig, MountType};

#[test]
fn test_valid_mounts() {
    let validator = EphemeralStorageValidator::new(HostConfig {
        total_memory_bytes: 1024 * 1024 * 1024, // 1GB
        max_tmpfs_percentage: 50, // 512MB max tmpfs
        total_ephemeral_storage_limit_bytes: 10 * 1024 * 1024 * 1024, // 10GB
        allocated_ephemeral_storage_bytes: 0,
    });

    let mounts = vec![
        MountConfig {
            mount_type: MountType::Tmpfs,
            target: "/tmp".to_string(),
            size_bytes: Some(100 * 1024 * 1024), // 100MB
        },
        MountConfig {
            mount_type: MountType::Ephemeral,
            target: "/var/lib/data".to_string(),
            size_bytes: Some(1024 * 1024 * 1024), // 1GB
        },
        MountConfig {
            mount_type: MountType::Bind,
            target: "/etc/config".to_string(),
            size_bytes: None,
        },
    ];

    let result = validator.validate_mounts(&mounts);
    assert!(result.is_ok());
}

#[test]
fn test_unbounded_tmpfs() {
    let validator = EphemeralStorageValidator::new(HostConfig {
        total_memory_bytes: 1024 * 1024 * 1024, // 1GB
        max_tmpfs_percentage: 50,
        total_ephemeral_storage_limit_bytes: 10 * 1024 * 1024 * 1024,
        allocated_ephemeral_storage_bytes: 0,
    });

    let mounts = vec![MountConfig {
        mount_type: MountType::Tmpfs,
        target: "/tmp".to_string(),
        size_bytes: None, // Missing size
    }];

    let result = validator.validate_mounts(&mounts);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.violations.len(), 1);
    assert_eq!(err.violations[0].resource_name, "Unbounded Tmpfs Mount: /tmp");
    assert_eq!(err.violations[0].limit, 512 * 1024 * 1024);
    assert_eq!(err.violations[0].requested, u64::MAX);
    assert_eq!(err.violations[0].remaining, 0);
}

#[test]
fn test_exceed_tmpfs_limit() {
    let validator = EphemeralStorageValidator::new(HostConfig {
        total_memory_bytes: 1024 * 1024 * 1024, // 1GB
        max_tmpfs_percentage: 10, // 102.4MB max tmpfs
        total_ephemeral_storage_limit_bytes: 10 * 1024 * 1024 * 1024,
        allocated_ephemeral_storage_bytes: 0,
    });

    let mounts = vec![
        MountConfig {
            mount_type: MountType::Tmpfs,
            target: "/tmp1".to_string(),
            size_bytes: Some(100 * 1024 * 1024),
        },
        MountConfig {
            mount_type: MountType::Tmpfs,
            target: "/tmp2".to_string(),
            size_bytes: Some(100 * 1024 * 1024),
        },
    ];

    let result = validator.validate_mounts(&mounts);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.violations.len(), 1);
    assert_eq!(err.violations[0].resource_name, "Total Tmpfs Size");
    assert_eq!(err.violations[0].limit, (1024 * 1024 * 1024 * 10) / 100);
    assert_eq!(err.violations[0].requested, 200 * 1024 * 1024);
}

#[test]
fn test_exceed_ephemeral_limit() {
    let validator = EphemeralStorageValidator::new(HostConfig {
        total_memory_bytes: 1024 * 1024 * 1024,
        max_tmpfs_percentage: 50,
        total_ephemeral_storage_limit_bytes: 2 * 1024 * 1024 * 1024, // 2GB
        allocated_ephemeral_storage_bytes: 1024 * 1024 * 1024, // 1GB already allocated
    });

    // Available is 1GB
    let mounts = vec![
        MountConfig {
            mount_type: MountType::Tmpfs,
            target: "/tmp".to_string(),
            size_bytes: Some(500 * 1024 * 1024),
        },
        MountConfig {
            mount_type: MountType::Ephemeral,
            target: "/var/lib/data".to_string(),
            size_bytes: Some(600 * 1024 * 1024),
        },
    ];

    let result = validator.validate_mounts(&mounts);
    assert!(result.is_err());
    let err = result.unwrap_err();
    // Tmpfs is within limit, but total ephemeral (tmpfs + ephemeral) = 1100MB > 1GB available
    assert_eq!(err.violations.len(), 1);
    assert_eq!(err.violations[0].resource_name, "Total Ephemeral Storage");
    assert_eq!(err.violations[0].limit, 1024 * 1024 * 1024);
    assert_eq!(err.violations[0].requested, 1100 * 1024 * 1024);
}

#[test]
fn test_multiple_violations() {
    let validator = EphemeralStorageValidator::new(HostConfig {
        total_memory_bytes: 1024 * 1024 * 1024, // 1GB
        max_tmpfs_percentage: 10, // 102.4MB
        total_ephemeral_storage_limit_bytes: 1024 * 1024 * 1024, // 1GB
        allocated_ephemeral_storage_bytes: 0,
    });

    let mounts = vec![
        MountConfig {
            mount_type: MountType::Tmpfs,
            target: "/tmp".to_string(),
            size_bytes: None,
        },
        MountConfig {
            mount_type: MountType::Tmpfs,
            target: "/var/run".to_string(),
            size_bytes: Some(200 * 1024 * 1024), // Exceeds tmpfs limit
        },
        MountConfig {
            mount_type: MountType::Ephemeral,
            target: "/data".to_string(),
            size_bytes: Some(2 * 1024 * 1024 * 1024), // Exceeds total limit
        },
    ];

    let result = validator.validate_mounts(&mounts);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.violations.len(), 3);
    assert_eq!(err.violations[0].resource_name, "Unbounded Tmpfs Mount: /tmp");
    assert_eq!(err.violations[1].resource_name, "Total Tmpfs Size");
    assert_eq!(err.violations[2].resource_name, "Total Ephemeral Storage");
}

#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MountType {
    Tmpfs,
    Ephemeral,
    Bind,
    PersistentVolume,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MountConfig {
    pub mount_type: MountType,
    pub target: String,
    pub size_bytes: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaViolation {
    pub resource_name: String,
    pub limit: u64,
    pub requested: u64,
    pub remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidationError {
    pub violations: Vec<QuotaViolation>,
}

#[derive(Debug, Clone)]
pub struct HostConfig {
    pub total_memory_bytes: u64,
    pub max_tmpfs_percentage: u8,
    pub total_ephemeral_storage_limit_bytes: u64,
    pub allocated_ephemeral_storage_bytes: u64,
}

#[derive(Debug, Clone)]
pub struct EphemeralStorageValidator {
    pub host_config: HostConfig,
}

impl EphemeralStorageValidator {
    pub fn new(host_config: HostConfig) -> Self {
        Self { host_config }
    }

    pub fn validate_mounts(&self, mounts: &[MountConfig]) -> Result<(), ValidationError> {
        let mut violations = Vec::new();

        let mut total_tmpfs_requested = 0u64;
        let mut total_ephemeral_requested = 0u64;

        let max_tmpfs_bytes = (self.host_config.total_memory_bytes as u128 * self.host_config.max_tmpfs_percentage as u128 / 100) as u64;
        let available_ephemeral = self.host_config.total_ephemeral_storage_limit_bytes.saturating_sub(self.host_config.allocated_ephemeral_storage_bytes);

        for mount in mounts {
            match mount.mount_type {
                MountType::Tmpfs => {
                    if let Some(size) = mount.size_bytes {
                        total_tmpfs_requested = total_tmpfs_requested.saturating_add(size);
                        total_ephemeral_requested = total_ephemeral_requested.saturating_add(size);
                    } else {
                        violations.push(QuotaViolation {
                            resource_name: format!("Unbounded Tmpfs Mount: {}", mount.target),
                            limit: max_tmpfs_bytes,
                            requested: u64::MAX,
                            remaining: 0,
                        });
                    }
                }
                MountType::Ephemeral => {
                    if let Some(size) = mount.size_bytes {
                        total_ephemeral_requested = total_ephemeral_requested.saturating_add(size);
                    } else {
                        violations.push(QuotaViolation {
                            resource_name: format!("Unbounded Ephemeral Mount: {}", mount.target),
                            limit: available_ephemeral,
                            requested: u64::MAX,
                            remaining: 0,
                        });
                    }
                }
                MountType::Bind | MountType::PersistentVolume => {}
            }
        }

        if total_tmpfs_requested > max_tmpfs_bytes {
            violations.push(QuotaViolation {
                resource_name: "Total Tmpfs Size".to_string(),
                limit: max_tmpfs_bytes,
                requested: total_tmpfs_requested,
                remaining: max_tmpfs_bytes.saturating_sub(total_tmpfs_requested),
            });
        }

        if total_ephemeral_requested > available_ephemeral {
            violations.push(QuotaViolation {
                resource_name: "Total Ephemeral Storage".to_string(),
                limit: available_ephemeral,
                requested: total_ephemeral_requested,
                remaining: available_ephemeral.saturating_sub(total_ephemeral_requested),
            });
        }

        if violations.is_empty() {
            Ok(())
        } else {
            Err(ValidationError { violations })
        }
    }
}

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum ResourceError {
    #[error("CPU quota is invalid (must be greater than 0)")]
    InvalidCpuQuota,
    #[error("Memory limit is invalid (must be greater than 0)")]
    InvalidMemoryLimit,
    #[error("CPU quota exceeds maximum allowed ({0} millicores)")]
    CpuQuotaExceeded(u32),
    #[error("Memory limit exceeds maximum allowed ({0} bytes)")]
    MemoryLimitExceeded(u64),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceLimits {
    pub cpu_millicores: u32,
    pub memory_bytes: u64,
}

impl ResourceLimits {
    // Sanity bounds
    pub const MAX_CPU_MILLICORES: u32 = 64_000; // 64 cores
    pub const MAX_MEMORY_BYTES: u64 = 256 * 1024 * 1024 * 1024; // 256 GB

    pub fn new(cpu_millicores: u32, memory_bytes: u64) -> Result<Self, ResourceError> {
        if cpu_millicores == 0 {
            return Err(ResourceError::InvalidCpuQuota);
        }
        if cpu_millicores > Self::MAX_CPU_MILLICORES {
            return Err(ResourceError::CpuQuotaExceeded(Self::MAX_CPU_MILLICORES));
        }

        if memory_bytes == 0 {
            return Err(ResourceError::InvalidMemoryLimit);
        }
        if memory_bytes > Self::MAX_MEMORY_BYTES {
            return Err(ResourceError::MemoryLimitExceeded(Self::MAX_MEMORY_BYTES));
        }

        Ok(Self {
            cpu_millicores,
            memory_bytes,
        })
    }
}

#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum PostgresCheckpointerError {
    #[error("Unsupported PostgreSQL version: {0}. Supported versions are 14, 15, 16, 17.")]
    UnsupportedVersion(u32),
    #[error("Invalid system specs: {0}")]
    InvalidSpecs(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PgVersion {
    V14,
    V15,
    V16,
    V17,
}

impl PgVersion {
    pub fn parse(version: u32) -> Result<Self, PostgresCheckpointerError> {
        match version {
            14 => Ok(PgVersion::V14),
            15 => Ok(PgVersion::V15),
            16 => Ok(PgVersion::V16),
            17 => Ok(PgVersion::V17),
            _ => Err(PostgresCheckpointerError::UnsupportedVersion(version)),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WorkloadProfile {
    ReadHeavy,
    Mixed,
    WriteHeavy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StorageType {
    Hdd,
    Ssd,
    Nvme,
}

#[derive(Debug, Clone)]
pub struct SystemSpecs {
    pub total_memory_mb: u64,
    pub storage_type: StorageType,
    pub disk_iops: u64,
    pub disk_throughput_mb_s: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PostgresCheckpointerConfig {
    pub checkpoint_timeout: u64, // seconds
    pub checkpoint_completion_target: f64,
    pub max_wal_size: u64, // MB
    pub min_wal_size: u64, // MB
    pub bgwriter_delay: u64, // ms
    pub bgwriter_lru_maxpages: u64,
    pub bgwriter_lru_multiplier: f64,
}

impl PostgresCheckpointerConfig {
    pub fn to_config_lines(&self) -> Vec<String> {
        vec![
            format!("checkpoint_timeout = {}s", self.checkpoint_timeout),
            format!("checkpoint_completion_target = {}", self.checkpoint_completion_target),
            format!("max_wal_size = {}MB", self.max_wal_size),
            format!("min_wal_size = {}MB", self.min_wal_size),
            format!("bgwriter_delay = {}ms", self.bgwriter_delay),
            format!("bgwriter_lru_maxpages = {}", self.bgwriter_lru_maxpages),
            format!("bgwriter_lru_multiplier = {}", self.bgwriter_lru_multiplier),
        ]
    }
}

pub struct PostgresCheckpointerTuner;

impl PostgresCheckpointerTuner {
    pub fn tune(
        _version: PgVersion,
        specs: &SystemSpecs,
        workload: WorkloadProfile,
    ) -> Result<PostgresCheckpointerConfig, PostgresCheckpointerError> {
        if specs.total_memory_mb < 512 {
            return Err(PostgresCheckpointerError::InvalidSpecs(
                "Total memory must be at least 512MB".to_string(),
            ));
        }
        if specs.disk_throughput_mb_s == 0 {
            return Err(PostgresCheckpointerError::InvalidSpecs(
                "Disk throughput must be > 0".to_string(),
            ));
        }

        // WAL spike prevention: smooth checkpoint I/O spikes across completion target window.
        let checkpoint_completion_target = 0.9;

        let (timeout, wal_pct) = match workload {
            WorkloadProfile::ReadHeavy => (900, 0.10),
            WorkloadProfile::Mixed => (900, 0.20),
            WorkloadProfile::WriteHeavy => (1800, 0.30),
        };

        let mut max_wal_size = (specs.total_memory_mb as f64 * wal_pct) as u64;

        // Bounds for max_wal_size based on workload
        max_wal_size = match workload {
            WorkloadProfile::ReadHeavy => max_wal_size.clamp(1024, 4096),
            WorkloadProfile::Mixed => max_wal_size.clamp(1024, 16384),
            WorkloadProfile::WriteHeavy => max_wal_size.clamp(2048, 65536),
        };

        // min_wal_size should typically be a fraction of max_wal_size, up to a reasonable cap.
        let min_wal_size = (max_wal_size / 4).clamp(512, 4096);

        // bgwriter parameter calculation based on storage capabilities
        let bgwriter_delay = match specs.storage_type {
            StorageType::Hdd => 200,
            StorageType::Ssd => 100,
            StorageType::Nvme => 20,
        };

        // Target bgwriter bandwidth is ~10% of available disk throughput
        let target_bgwriter_mb_s = (specs.disk_throughput_mb_s as f64 * 0.10).max(1.0);

        // 1 page = 8KB, so 1 MB = 128 pages
        let pages_per_second = target_bgwriter_mb_s * 128.0;

        // bgwriter runs every `bgwriter_delay` ms
        let runs_per_second = 1000.0 / bgwriter_delay as f64;

        // Calculate maxpages per run
        let bgwriter_lru_maxpages = ((pages_per_second / runs_per_second) as u64).clamp(100, 10000);

        let bgwriter_lru_multiplier = match workload {
            WorkloadProfile::ReadHeavy => 2.0,
            WorkloadProfile::Mixed => 2.5,
            WorkloadProfile::WriteHeavy => 3.5,
        };

        Ok(PostgresCheckpointerConfig {
            checkpoint_timeout: timeout,
            checkpoint_completion_target,
            max_wal_size,
            min_wal_size,
            bgwriter_delay,
            bgwriter_lru_maxpages,
            bgwriter_lru_multiplier,
        })
    }
}

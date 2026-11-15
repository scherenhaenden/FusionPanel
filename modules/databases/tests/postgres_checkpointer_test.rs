#![forbid(unsafe_code)]

use databases::postgres_checkpointer::{
    PgVersion, PostgresCheckpointerConfig, PostgresCheckpointerError, PostgresCheckpointerTuner,
    StorageType, SystemSpecs, WorkloadProfile,
};

#[test]
fn test_version_parsing() {
    assert_eq!(PgVersion::parse(14).unwrap(), PgVersion::V14);
    assert_eq!(PgVersion::parse(15).unwrap(), PgVersion::V15);
    assert_eq!(PgVersion::parse(16).unwrap(), PgVersion::V16);
    assert_eq!(PgVersion::parse(17).unwrap(), PgVersion::V17);
    assert_eq!(
        PgVersion::parse(13).unwrap_err(),
        PostgresCheckpointerError::UnsupportedVersion(13)
    );
}

#[test]
fn test_invalid_specs() {
    let specs_low_mem = SystemSpecs {
        total_memory_mb: 256,
        storage_type: StorageType::Ssd,
        disk_iops: 5000,
        disk_throughput_mb_s: 200,
    };

    assert!(matches!(
        PostgresCheckpointerTuner::tune(PgVersion::V16, &specs_low_mem, WorkloadProfile::Mixed),
        Err(PostgresCheckpointerError::InvalidSpecs(_))
    ));

    let specs_zero_throughput = SystemSpecs {
        total_memory_mb: 4096,
        storage_type: StorageType::Ssd,
        disk_iops: 5000,
        disk_throughput_mb_s: 0,
    };

    assert!(matches!(
        PostgresCheckpointerTuner::tune(PgVersion::V16, &specs_zero_throughput, WorkloadProfile::Mixed),
        Err(PostgresCheckpointerError::InvalidSpecs(_))
    ));
}

#[test]
fn test_tune_ssd_mixed_16gb() {
    let specs = SystemSpecs {
        total_memory_mb: 16384,
        storage_type: StorageType::Ssd,
        disk_iops: 5000,
        disk_throughput_mb_s: 200,
    };

    let config = PostgresCheckpointerTuner::tune(PgVersion::V16, &specs, WorkloadProfile::Mixed).unwrap();

    assert_eq!(config.checkpoint_timeout, 900);
    assert_eq!(config.checkpoint_completion_target, 0.9);

    // mixed max_wal_size: 16384 * 0.20 = 3276.8 -> 3276
    assert_eq!(config.max_wal_size, 3276);

    // min_wal_size: 3276 / 4 = 819
    assert_eq!(config.min_wal_size, 819);

    // ssd bgwriter_delay
    assert_eq!(config.bgwriter_delay, 100);

    // bgwriter_lru_maxpages:
    // target bgwriter = 200 * 0.10 = 20 MB/s
    // pages per second = 20 * 128 = 2560
    // runs per second = 1000 / 100 = 10
    // maxpages = 2560 / 10 = 256
    assert_eq!(config.bgwriter_lru_maxpages, 256);

    assert_eq!(config.bgwriter_lru_multiplier, 2.5);
}

#[test]
fn test_tune_nvme_write_heavy_128gb() {
    let specs = SystemSpecs {
        total_memory_mb: 131072, // 128GB
        storage_type: StorageType::Nvme,
        disk_iops: 100000,
        disk_throughput_mb_s: 3000,
    };

    let config = PostgresCheckpointerTuner::tune(PgVersion::V16, &specs, WorkloadProfile::WriteHeavy).unwrap();

    assert_eq!(config.checkpoint_timeout, 1800);

    // write heavy max_wal_size: 131072 * 0.30 = 39321.6 -> 39321
    assert_eq!(config.max_wal_size, 39321);

    // min_wal_size: 39321 / 4 = 9830. Clamp max 4096.
    assert_eq!(config.min_wal_size, 4096);

    // nvme bgwriter_delay
    assert_eq!(config.bgwriter_delay, 20);

    // target bgwriter = 3000 * 0.10 = 300 MB/s
    // pages per second = 300 * 128 = 38400
    // runs per second = 1000 / 20 = 50
    // maxpages = 38400 / 50 = 768
    assert_eq!(config.bgwriter_lru_maxpages, 768);

    assert_eq!(config.bgwriter_lru_multiplier, 3.5);
}

#[test]
fn test_tune_hdd_read_heavy_4gb() {
    let specs = SystemSpecs {
        total_memory_mb: 4096,
        storage_type: StorageType::Hdd,
        disk_iops: 200,
        disk_throughput_mb_s: 80,
    };

    let config = PostgresCheckpointerTuner::tune(PgVersion::V16, &specs, WorkloadProfile::ReadHeavy).unwrap();

    // max_wal_size: 4096 * 0.10 = 409. Clamped min is 1024.
    assert_eq!(config.max_wal_size, 1024);

    // min_wal_size: 1024 / 4 = 256. Clamped min is 512.
    assert_eq!(config.min_wal_size, 512);

    assert_eq!(config.bgwriter_delay, 200);

    // bgwriter_lru_maxpages
    // target bgwriter = 80 * 0.10 = 8 MB/s
    // pages/s = 8 * 128 = 1024
    // runs/s = 1000 / 200 = 5
    // maxpages = 1024 / 5 = 204. Clamped to 204 (min is 100)
    assert_eq!(config.bgwriter_lru_maxpages, 204);

    assert_eq!(config.bgwriter_lru_multiplier, 2.0);
}

#[test]
fn test_config_lines_formatting() {
    let config = PostgresCheckpointerConfig {
        checkpoint_timeout: 900,
        checkpoint_completion_target: 0.9,
        max_wal_size: 1024,
        min_wal_size: 512,
        bgwriter_delay: 100,
        bgwriter_lru_maxpages: 200,
        bgwriter_lru_multiplier: 2.0,
    };

    let lines = config.to_config_lines();
    assert_eq!(lines.len(), 7);
    assert_eq!(lines[0], "checkpoint_timeout = 900s");
    assert_eq!(lines[1], "checkpoint_completion_target = 0.9");
    assert_eq!(lines[2], "max_wal_size = 1024MB");
    assert_eq!(lines[3], "min_wal_size = 512MB");
    assert_eq!(lines[4], "bgwriter_delay = 100ms");
    assert_eq!(lines[5], "bgwriter_lru_maxpages = 200");
    assert_eq!(lines[6], "bgwriter_lru_multiplier = 2"); // Note: f64 format might drop .0
}

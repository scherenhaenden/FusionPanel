#![forbid(unsafe_code)]

use backup_destinations::retention_pruner::{BackupArchive, RetentionPolicy, RetentionPolicyEngine};
use chrono::{TimeZone, Utc};

#[test]
fn test_retention_pruner_basic_daily() {
    let policy = RetentionPolicy {
        keep_daily: 3,
        keep_weekly: 0,
        keep_monthly: 0,
        keep_yearly: 0,
    };

    let backups = vec![
        BackupArchive { id: "1".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 5, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "2".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 4, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "3".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 3, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "4".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 2, 12, 0, 0).unwrap(), destination: "local".into() },
    ];

    let result = RetentionPolicyEngine::evaluate(&policy, backups);
    assert_eq!(result.retain.len(), 3);
    assert_eq!(result.prune.len(), 1);
    assert_eq!(result.retain[0].id, "1");
    assert_eq!(result.retain[1].id, "2");
    assert_eq!(result.retain[2].id, "3");
    assert_eq!(result.prune[0].id, "4");
}

#[test]
fn test_retention_pruner_multiple_per_day() {
    let policy = RetentionPolicy {
        keep_daily: 2,
        keep_weekly: 0,
        keep_monthly: 0,
        keep_yearly: 0,
    };

    let backups = vec![
        BackupArchive { id: "1".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 5, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "2".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 5, 10, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "3".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 4, 12, 0, 0).unwrap(), destination: "local".into() },
    ];

    let result = RetentionPolicyEngine::evaluate(&policy, backups);

    assert_eq!(result.retain.len(), 2);
    assert_eq!(result.prune.len(), 1);
    assert_eq!(result.retain[0].id, "1");
    assert_eq!(result.retain[1].id, "3");
    assert_eq!(result.prune[0].id, "2");
}

#[test]
fn test_retention_pruner_gfs_leap_year_and_boundaries() {
    let policy = RetentionPolicy {
        keep_daily: 7,
        keep_weekly: 4,
        keep_monthly: 12,
        keep_yearly: 2,
    };

    let backups = vec![
        BackupArchive { id: "y2".into(), timestamp: Utc.with_ymd_and_hms(2024, 2, 29, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "y1".into(), timestamp: Utc.with_ymd_and_hms(2023, 12, 31, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "y0".into(), timestamp: Utc.with_ymd_and_hms(2022, 1, 1, 12, 0, 0).unwrap(), destination: "local".into() },
    ];

    let result = RetentionPolicyEngine::evaluate(&policy, backups);

    let retain_ids: Vec<String> = result.retain.into_iter().map(|b| b.id).collect();
    let prune_ids: Vec<String> = result.prune.into_iter().map(|b| b.id).collect();

    assert_eq!(retain_ids.len(), 3);
    assert_eq!(prune_ids.len(), 0);
    assert!(retain_ids.contains(&"y2".into()));
    assert!(retain_ids.contains(&"y1".into()));
    assert!(retain_ids.contains(&"y0".into()));
}

#[test]
fn test_retention_pruner_gfs_mixed() {
    let policy = RetentionPolicy {
        keep_daily: 2,
        keep_weekly: 1,
        keep_monthly: 2,
        keep_yearly: 2,
    };

    let backups = vec![
        BackupArchive { id: "d1".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 5, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "d2".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 4, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "d3".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 3, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "w1".into(), timestamp: Utc.with_ymd_and_hms(2023, 9, 28, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "m1".into(), timestamp: Utc.with_ymd_and_hms(2023, 8, 15, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "y1".into(), timestamp: Utc.with_ymd_and_hms(2022, 5, 10, 12, 0, 0).unwrap(), destination: "local".into() },
        BackupArchive { id: "old".into(), timestamp: Utc.with_ymd_and_hms(2021, 5, 10, 12, 0, 0).unwrap(), destination: "local".into() },
    ];

    let result = RetentionPolicyEngine::evaluate(&policy, backups);

    let retain_ids: Vec<String> = result.retain.into_iter().map(|b| b.id).collect();
    let prune_ids: Vec<String> = result.prune.into_iter().map(|b| b.id).collect();

    assert!(retain_ids.contains(&"d1".into()));
    assert!(retain_ids.contains(&"d2".into()));
    assert!(prune_ids.contains(&"d3".into()));
    assert!(retain_ids.contains(&"w1".into()));
    assert!(prune_ids.contains(&"m1".into()));
    assert!(retain_ids.contains(&"y1".into()));
    assert!(prune_ids.contains(&"old".into()));
}

#[test]
fn test_retention_pruner_zero_policy() {
    let policy = RetentionPolicy {
        keep_daily: 0,
        keep_weekly: 0,
        keep_monthly: 0,
        keep_yearly: 0,
    };

    let backups = vec![
        BackupArchive { id: "1".into(), timestamp: Utc.with_ymd_and_hms(2023, 10, 5, 12, 0, 0).unwrap(), destination: "local".into() }
    ];

    let result = RetentionPolicyEngine::evaluate(&policy, backups);
    assert_eq!(result.retain.len(), 0);
    assert_eq!(result.prune.len(), 1);
}

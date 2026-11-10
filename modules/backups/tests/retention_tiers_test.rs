#![forbid(unsafe_code)]

use chrono::{TimeZone, Utc};
use backups::retention_tiers::{GfsRetentionPlanner, GfsRetentionPolicy};

#[derive(Clone, Debug, PartialEq)]
struct Snapshot {
    id: usize,
}

#[test]
fn test_retention_policy_creation() {
    let policy = GfsRetentionPolicy::new(7, 4, 12, 3).unwrap();
    assert_eq!(policy.keep_daily, 7);
    assert_eq!(policy.keep_weekly, 4);
    assert_eq!(policy.keep_monthly, 12);
    assert_eq!(policy.keep_yearly, 3);
}

#[test]
fn test_always_keep_newest_snapshot() {
    let policy = GfsRetentionPolicy::new(0, 0, 0, 0).unwrap(); // keep nothing

    // Nov 2 2026, 12:00:00
    let dt1 = Utc.with_ymd_and_hms(2026, 11, 2, 12, 0, 0).unwrap();
    let dt2 = Utc.with_ymd_and_hms(2026, 11, 1, 12, 0, 0).unwrap();

    let snapshots = vec![
        (Snapshot { id: 1 }, dt1),
        (Snapshot { id: 2 }, dt2),
    ];

    let result = GfsRetentionPlanner::plan_retention(&snapshots, &policy);

    assert_eq!(result.retained.len(), 1);
    assert_eq!(result.retained[0].id, 1); // dt1 is newest

    assert_eq!(result.pruned.len(), 1);
    assert_eq!(result.pruned[0].id, 2);
}

#[test]
fn test_retention_over_multi_month() {
    let policy = GfsRetentionPolicy::new(2, 2, 2, 1).unwrap();

    let snapshots = vec![
        // December 2026
        (Snapshot { id: 1 }, Utc.with_ymd_and_hms(2026, 12, 10, 12, 0, 0).unwrap()),
        (Snapshot { id: 2 }, Utc.with_ymd_and_hms(2026, 12, 9, 12, 0, 0).unwrap()),
        (Snapshot { id: 3 }, Utc.with_ymd_and_hms(2026, 12, 8, 12, 0, 0).unwrap()), // pruned daily

        // November 2026
        (Snapshot { id: 4 }, Utc.with_ymd_and_hms(2026, 11, 30, 12, 0, 0).unwrap()),
        (Snapshot { id: 5 }, Utc.with_ymd_and_hms(2026, 11, 29, 12, 0, 0).unwrap()), // pruned monthly/weekly
        (Snapshot { id: 6 }, Utc.with_ymd_and_hms(2026, 11, 15, 12, 0, 0).unwrap()),
    ];

    let result = GfsRetentionPlanner::plan_retention(&snapshots, &policy);

    // Let's trace it:
    // newest: 2026-12-10 (id: 1) -> daily (1/2), weekly (1/2), monthly (1/2), yearly (1/1)
    // 2026-12-09 (id: 2) -> daily (2/2)
    // 2026-12-08 (id: 3) -> daily full, weekly (1/2 - same week as 1? No, 12-10 is Thursday W50. 12-08 is Tuesday W50. So same week). monthly (same), yearly (same). -> Pruned!
    // 2026-11-30 (id: 4) -> daily full, weekly (W49) (2/2), monthly (2/2), yearly full. -> Kept!
    // 2026-11-29 (id: 5) -> daily full, weekly (W48) (no more weeklies), monthly (same). -> Pruned!
    // 2026-11-15 (id: 6) -> daily full, weekly full, monthly full, yearly full. -> Pruned!

    let retained_ids: Vec<usize> = result.retained.iter().map(|s| s.id).collect();
    let pruned_ids: Vec<usize> = result.pruned.iter().map(|s| s.id).collect();

    assert!(retained_ids.contains(&1));
    assert!(retained_ids.contains(&2));
    assert!(pruned_ids.contains(&3));
    assert!(retained_ids.contains(&4));
    assert!(pruned_ids.contains(&5));
    assert!(pruned_ids.contains(&6));
}

#[test]
fn test_no_snapshots() {
    let policy = GfsRetentionPolicy::new(7, 4, 12, 3).unwrap();
    let snapshots: Vec<(Snapshot, chrono::DateTime<chrono::Utc>)> = vec![];

    let result = GfsRetentionPlanner::plan_retention(&snapshots, &policy);

    assert!(result.retained.is_empty());
    assert!(result.pruned.is_empty());
}

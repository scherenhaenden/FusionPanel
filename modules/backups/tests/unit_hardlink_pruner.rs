#![forbid(unsafe_code)]

use backups::{
    HardlinkRetentionPolicy, HardlinkSnapshotInventory,
};
use chrono::{Duration, Utc};
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_hardlink_deduplication_and_savings() {
    let dir = tempdir().unwrap();
    let base = dir.path();

    let snap1 = base.join("snap-2026-11-01");
    let snap2 = base.join("snap-2026-11-02");
    let snap3 = base.join("snap-2026-11-03");

    fs::create_dir(&snap1).unwrap();
    fs::create_dir(&snap2).unwrap();
    fs::create_dir(&snap3).unwrap();

    // snap1 files
    let file1_content = vec![b'A'; 1024];
    let file2_content = vec![b'B'; 2048];
    File::create(snap1.join("file1.txt"))
        .unwrap()
        .write_all(&file1_content)
        .unwrap();
    File::create(snap1.join("file2.txt"))
        .unwrap()
        .write_all(&file2_content)
        .unwrap();

    // snap2: hardlink file1, new file3
    fs::hard_link(snap1.join("file1.txt"), snap2.join("file1.txt")).unwrap();
    let file3_content = vec![b'C'; 512];
    File::create(snap2.join("file3.txt"))
        .unwrap()
        .write_all(&file3_content)
        .unwrap();

    // snap3: hardlink file1 and file2, new file4
    fs::hard_link(snap1.join("file1.txt"), snap3.join("file1.txt")).unwrap();
    fs::hard_link(snap1.join("file2.txt"), snap3.join("file2.txt")).unwrap();
    let file4_content = vec![b'D'; 100];
    File::create(snap3.join("file4.txt"))
        .unwrap()
        .write_all(&file4_content)
        .unwrap();

    let inventory = HardlinkSnapshotInventory::scan_directory(base).unwrap();
    assert_eq!(inventory.snapshots().len(), 3);

    let savings = inventory.calculate_space_savings();

    // Apparent bytes:
    // snap1: 1024 + 2048 = 3072
    // snap2: 1024 + 512 = 1536
    // snap3: 1024 + 2048 + 100 = 3172
    // Total apparent: 3072 + 1536 + 3172 = 7780
    assert_eq!(savings.total_apparent_bytes, 7780);

    // Unique physical bytes: 1024 + 2048 + 512 + 100 = 3684
    assert_eq!(savings.unique_physical_bytes, 3684);

    // Saved bytes: 7780 - 3684 = 4096
    assert_eq!(savings.saved_bytes, 4096);
    assert!((savings.deduplication_ratio - (7780.0 / 3684.0)).abs() < 0.0001);
}

#[test]
fn test_reclaimable_bytes_calculation() {
    let dir = tempdir().unwrap();
    let base = dir.path();

    let snap1 = base.join("snap-1");
    let snap2 = base.join("snap-2");
    let snap3 = base.join("snap-3");

    fs::create_dir(&snap1).unwrap();
    fs::create_dir(&snap2).unwrap();
    fs::create_dir(&snap3).unwrap();

    // Shared between snap1 and snap3 (1000 bytes)
    File::create(snap1.join("shared13.bin"))
        .unwrap()
        .write_all(&vec![1u8; 1000])
        .unwrap();
    fs::hard_link(snap1.join("shared13.bin"), snap3.join("shared13.bin")).unwrap();

    // Unique to snap1 (600 bytes)
    File::create(snap1.join("unique1.bin"))
        .unwrap()
        .write_all(&vec![2u8; 600])
        .unwrap();

    // Unique to snap2 (400 bytes)
    File::create(snap2.join("unique2.bin"))
        .unwrap()
        .write_all(&vec![3u8; 400])
        .unwrap();

    // Set mtime to ensure deterministic ordering (snap3 newest, snap1 oldest)
    let now = Utc::now();
    set_mtime(&snap3, now);
    set_mtime(&snap2, now - Duration::hours(1));
    set_mtime(&snap1, now - Duration::hours(2));

    let inventory = HardlinkSnapshotInventory::scan_directory(base).unwrap();

    // Policy: keep only the newest 1 snapshot (snap3)
    let policy = HardlinkRetentionPolicy {
        keep_last: Some(1),
        keep_daily: None,
        keep_weekly: None,
        keep_monthly: None,
        max_age_days: None,
    };

    let plan = inventory.plan_pruning(&policy, now);
    assert_eq!(plan.retained.len(), 1);
    assert_eq!(plan.retained[0].name, "snap-3");
    assert_eq!(plan.to_prune.len(), 2);

    // Pruning snap1 and snap2:
    // shared13.bin is retained in snap3 -> NOT reclaimable
    // unique1.bin (600) + unique2.bin (400) -> 1000 bytes reclaimable!
    assert_eq!(plan.reclaimable_bytes, 1000);
}

#[test]
fn test_dry_run_and_execution() {
    let dir = tempdir().unwrap();
    let base = dir.path();

    let snap_old = base.join("snap-old");
    let snap_new = base.join("snap-new");

    fs::create_dir(&snap_old).unwrap();
    fs::create_dir(&snap_new).unwrap();

    File::create(snap_old.join("old.log"))
        .unwrap()
        .write_all(b"old data")
        .unwrap();
    File::create(snap_new.join("new.log"))
        .unwrap()
        .write_all(b"new data")
        .unwrap();

    let now = Utc::now();
    set_mtime(&snap_new, now);
    set_mtime(&snap_old, now - Duration::days(5));

    let mut inventory = HardlinkSnapshotInventory::scan_directory(base).unwrap();
    let policy = HardlinkRetentionPolicy {
        keep_last: Some(1),
        ..Default::default()
    };

    let plan = inventory.plan_pruning(&policy, now);
    assert_eq!(plan.to_prune.len(), 1);

    // Dry run
    let dry_res = inventory.execute_pruning(&plan, true).unwrap();
    assert!(dry_res.dry_run);
    assert!(snap_old.exists(), "Snapshot must remain after dry run");
    assert!(snap_new.exists());

    // Live run
    let live_res = inventory.execute_pruning(&plan, false).unwrap();
    assert!(!live_res.dry_run);
    assert!(!snap_old.exists(), "Pruned snapshot must be deleted");
    assert!(snap_new.exists(), "Retained snapshot must remain intact");
    assert_eq!(inventory.snapshots().len(), 1);
}

#[test]
fn test_retention_policy_daily_and_max_age() {
    let dir = tempdir().unwrap();
    let base = dir.path();

    let now = Utc::now();

    // Create snapshots over several days
    for i in 0..6 {
        let name = format!("snap-day-{}", i);
        let path = base.join(&name);
        fs::create_dir(&path).unwrap();
        File::create(path.join("file.txt"))
            .unwrap()
            .write_all(b"test")
            .unwrap();
        set_mtime(&path, now - Duration::days(i));
    }

    let inventory = HardlinkSnapshotInventory::scan_directory(base).unwrap();
    assert_eq!(inventory.snapshots().len(), 6);

    // Policy: keep daily for 3 days, max age 4 days
    let policy = HardlinkRetentionPolicy {
        keep_last: Some(1),
        keep_daily: Some(3),
        keep_weekly: None,
        keep_monthly: None,
        max_age_days: Some(4),
    };

    let plan = inventory.plan_pruning(&policy, now);
    // snap-day-0 (now): retained (keep_last + daily)
    // snap-day-1: retained (daily)
    // snap-day-2: retained (daily)
    // snap-day-3: retained (daily)
    // snap-day-4: pruned
    // snap-day-5: pruned
    assert_eq!(plan.retained.len(), 4);
    assert_eq!(plan.to_prune.len(), 2);
}

fn set_mtime(path: &Path, time: chrono::DateTime<Utc>) {
    let filetime = filetime::FileTime::from_unix_time(time.timestamp(), 0);
    filetime::set_file_mtime(path, filetime).unwrap();
}

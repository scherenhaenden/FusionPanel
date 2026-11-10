#![forbid(unsafe_code)]

use containers::overlay_pruner::{OverlayPruner, PrunePlan};
use std::collections::HashSet;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_overlay_pruner_no_orphans() {
    let overlay2_dir = tempdir().unwrap();
    let shm_dir = tempdir().unwrap();

    let pruner = OverlayPruner::new(overlay2_dir.path(), shm_dir.path());

    let mut active_overlay = HashSet::new();
    let mut active_shm = HashSet::new();

    // Create an active overlay dir
    let active_overlay_path = overlay2_dir.path().join("active_layer");
    fs::create_dir(&active_overlay_path).unwrap();
    active_overlay.insert("active_layer".to_string());

    // Create an active shm dir
    let active_shm_path = shm_dir.path().join("active_shm");
    fs::create_dir(&active_shm_path).unwrap();
    active_shm.insert("active_shm".to_string());

    let plan = pruner.plan_prune(&active_overlay, &active_shm).unwrap();

    assert_eq!(plan.reclaimable_bytes, 0);
    assert!(plan.orphaned_overlay_dirs.is_empty());
    assert!(plan.orphaned_shm_dirs.is_empty());
}

#[test]
fn test_overlay_pruner_with_orphans() {
    let overlay2_dir = tempdir().unwrap();
    let shm_dir = tempdir().unwrap();

    let pruner = OverlayPruner::new(overlay2_dir.path(), shm_dir.path());

    let mut active_overlay = HashSet::new();
    let mut active_shm = HashSet::new();

    // Active
    let active_overlay_path = overlay2_dir.path().join("active_layer");
    fs::create_dir(&active_overlay_path).unwrap();
    active_overlay.insert("active_layer".to_string());

    let active_shm_path = shm_dir.path().join("active_shm");
    fs::create_dir(&active_shm_path).unwrap();
    active_shm.insert("active_shm".to_string());

    // Orphaned
    let orphan_overlay_path = overlay2_dir.path().join("orphan_layer");
    fs::create_dir(&orphan_overlay_path).unwrap();
    fs::write(orphan_overlay_path.join("file.txt"), b"12345").unwrap();

    let orphan_shm_path = shm_dir.path().join("orphan_shm");
    fs::create_dir(&orphan_shm_path).unwrap();
    fs::write(orphan_shm_path.join("data.bin"), b"123").unwrap();

    let plan = pruner.plan_prune(&active_overlay, &active_shm).unwrap();

    // Verify
    assert!(plan.reclaimable_bytes > 0);
    assert_eq!(plan.orphaned_overlay_dirs.len(), 1);
    assert_eq!(plan.orphaned_overlay_dirs[0], orphan_overlay_path);
    assert_eq!(plan.orphaned_shm_dirs.len(), 1);
    assert_eq!(plan.orphaned_shm_dirs[0], orphan_shm_path);
}

#[test]
fn test_overlay_pruner_skips_special_dirs() {
    let overlay2_dir = tempdir().unwrap();
    let shm_dir = tempdir().unwrap();

    let pruner = OverlayPruner::new(overlay2_dir.path(), shm_dir.path());

    let active_overlay = HashSet::new();
    let active_shm = HashSet::new();

    // 'l' and 'backingFsBlockDev'
    fs::create_dir(overlay2_dir.path().join("l")).unwrap();
    fs::write(overlay2_dir.path().join("backingFsBlockDev"), b"blockdev").unwrap();

    let plan = pruner.plan_prune(&active_overlay, &active_shm).unwrap();

    assert_eq!(plan.reclaimable_bytes, 0);
    assert!(plan.orphaned_overlay_dirs.is_empty());
}

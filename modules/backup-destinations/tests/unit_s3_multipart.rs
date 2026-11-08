#![forbid(unsafe_code)]

use backup_destinations::s3_multipart::{
    ResumablePartTracker, S3MultipartPlanner, MAX_OBJECT_SIZE, MAX_PARTS, MAX_PART_SIZE,
    MIN_PART_SIZE,
};

#[test]
fn test_small_archive() {
    let size = 1024 * 1024; // 1 MiB
    let planner = S3MultipartPlanner::new(size).unwrap();
    assert_eq!(planner.parts.len(), 1);
    assert_eq!(planner.parts[0].part_number, 1);
    assert_eq!(planner.parts[0].offset, 0);
    assert_eq!(planner.parts[0].length, size);
}

#[test]
fn test_standard_archive() {
    let size = 15 * 1024 * 1024; // 15 MiB
    let planner = S3MultipartPlanner::new(size).unwrap();
    assert_eq!(planner.parts.len(), 3);
    assert_eq!(planner.parts[0].length, 5 * 1024 * 1024);
    assert_eq!(planner.parts[1].length, 5 * 1024 * 1024);
    assert_eq!(planner.parts[2].length, 5 * 1024 * 1024);
}

#[test]
fn test_large_archive() {
    let size = 150 * 1024 * 1024 * 1024; // 150 GiB
    let planner = S3MultipartPlanner::new(size).unwrap();
    assert!(planner.parts.len() as u32 <= MAX_PARTS);
    for part in &planner.parts {
        assert!(part.length <= MAX_PART_SIZE);
        assert!(part.length >= MIN_PART_SIZE || part.part_number == planner.parts.len() as u32);
    }
}

#[test]
fn test_exceeds_max_object_size() {
    let size = MAX_OBJECT_SIZE + 1;
    let err = S3MultipartPlanner::new(size).unwrap_err();
    assert!(err.contains("Total size exceeds maximum S3 object size"));
}

#[test]
fn test_zero_size_archive() {
    let planner = S3MultipartPlanner::new(0).unwrap();
    assert_eq!(planner.parts.len(), 1);
    assert_eq!(planner.parts[0].length, 0);
}

#[test]
fn test_part_tracker_complete() {
    let size = 15 * 1024 * 1024; // 15 MiB
    let planner = S3MultipartPlanner::new(size).unwrap();
    let mut tracker = ResumablePartTracker::new();

    for part in &planner.parts {
        tracker.record_part(part.part_number, format!("etag{}", part.part_number));
    }

    assert!(tracker.is_complete(&planner));
    let manifest = tracker.verify_manifest(&planner).unwrap();
    assert_eq!(manifest.len(), 3);
}

#[test]
fn test_part_tracker_missing_parts() {
    let size = 15 * 1024 * 1024; // 15 MiB
    let planner = S3MultipartPlanner::new(size).unwrap();
    let mut tracker = ResumablePartTracker::new();

    tracker.record_part(1, "etag1".to_string());
    tracker.record_part(3, "etag3".to_string());

    assert!(!tracker.is_complete(&planner));
    let remaining = tracker.remaining_parts(&planner);
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].part_number, 2);

    let err = tracker.verify_manifest(&planner).unwrap_err();
    assert!(err.contains("missing parts"));
}

#![forbid(unsafe_code)]

use containers::volume_cloner::{SnapshotMode, VolumeCloner};
use std::fs;
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

#[test]
fn test_plan_snapshot() {
    let mode = SnapshotMode::ReadOnlyBind;
    let path = Path::new("/var/lib/docker/volumes/my_volume");
    let plan = VolumeCloner::plan_snapshot("my_volume", path, mode.clone());

    assert_eq!(plan.volume_name, "my_volume");
    assert_eq!(plan.source_path, path);
    assert_eq!(plan.mode, mode);
}

#[test]
fn test_generate_manifest() {
    let temp_dir = TempDir::new().unwrap();
    let root = temp_dir.path();

    let file1_path = root.join("file1.txt");
    fs::write(&file1_path, "hello").unwrap();

    let sub_dir = root.join("subdir");
    fs::create_dir(&sub_dir).unwrap();

    let file2_path = sub_dir.join("file2.txt");
    fs::write(&file2_path, "world").unwrap();

    let manifest = VolumeCloner::generate_manifest(root).unwrap();

    assert_eq!(manifest.files.len(), 2);
    assert!(manifest.files.contains_key(Path::new("file1.txt")));
    assert!(manifest.files.contains_key(Path::new("subdir/file2.txt")));
    assert_eq!(manifest.files.get(Path::new("file1.txt")).unwrap().size, 5);
    assert_eq!(
        manifest
            .files
            .get(Path::new("subdir/file2.txt"))
            .unwrap()
            .size,
        5
    );
}

#[test]
fn test_calculate_difference() {
    let temp_dir_old = TempDir::new().unwrap();
    let old_root = temp_dir_old.path();

    fs::write(old_root.join("file_kept.txt"), "kept").unwrap();
    fs::write(old_root.join("file_removed.txt"), "remove_me").unwrap();
    fs::write(old_root.join("file_modified.txt"), "old_content").unwrap();

    let old_manifest = VolumeCloner::generate_manifest(old_root).unwrap();

    let temp_dir_new = TempDir::new().unwrap();
    let new_root = temp_dir_new.path();

    fs::write(new_root.join("file_kept.txt"), "kept").unwrap();
    // sleep briefly so modified time changes (or we rely on size diff)
    std::thread::sleep(Duration::from_millis(10));
    fs::write(new_root.join("file_modified.txt"), "new_content_bigger").unwrap();
    fs::write(new_root.join("file_added.txt"), "added").unwrap();

    let new_manifest = VolumeCloner::generate_manifest(new_root).unwrap();

    let diff = VolumeCloner::calculate_difference(&old_manifest, &new_manifest);

    assert_eq!(diff.files_removed.len(), 1);
    assert!(diff
        .files_removed
        .contains(&Path::new("file_removed.txt").to_path_buf()));

    assert_eq!(diff.files_added.len(), 1);
    assert!(diff
        .files_added
        .contains(&Path::new("file_added.txt").to_path_buf()));

    assert_eq!(diff.files_modified.len(), 1);
    assert!(diff
        .files_modified
        .contains(&Path::new("file_modified.txt").to_path_buf()));

    let size_removed = -("remove_me".len() as i64);
    let size_added = "added".len() as i64;
    let size_modified = ("new_content_bigger".len() as i64) - ("old_content".len() as i64);
    assert_eq!(diff.size_diff, size_removed + size_added + size_modified);
}

#[test]
fn test_create_cow_clone() {
    let temp_dir = TempDir::new().unwrap();
    let source_root = temp_dir.path().join("source");
    let dest_root = temp_dir.path().join("dest");

    fs::create_dir(&source_root).unwrap();
    fs::write(source_root.join("file1.txt"), "content").unwrap();
    let subdir = source_root.join("subdir");
    fs::create_dir(&subdir).unwrap();
    fs::write(subdir.join("file2.txt"), "more content").unwrap();

    VolumeCloner::create_cow_clone(&source_root, &dest_root).unwrap();

    // Check files exist in dest
    assert!(dest_root.join("file1.txt").exists());
    assert!(dest_root.join("subdir/file2.txt").exists());
    assert_eq!(
        fs::read_to_string(dest_root.join("file1.txt")).unwrap(),
        "content"
    );

    // Check hard links
    let src_metadata = fs::metadata(source_root.join("file1.txt")).unwrap();
    let dest_metadata = fs::metadata(dest_root.join("file1.txt")).unwrap();

    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        assert_eq!(src_metadata.ino(), dest_metadata.ino());
        assert_eq!(src_metadata.nlink(), 2);
    }
}

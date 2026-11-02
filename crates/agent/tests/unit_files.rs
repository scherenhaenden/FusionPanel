use agent::files::FileManager;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::tempdir;

#[test]
fn test_write_file_atomic() {
    let dir = tempdir().unwrap();
    let file_path = dir.path().join("test_file.txt");
    let manager = FileManager::new();

    let content = b"hello world";

    // We cannot reliably test uid/gid changes unless root, so test None for them.
    // Testing mode changes to something readable/writable.
    manager
        .write_file_atomic(&file_path, content, Some(0o644), None, None)
        .unwrap();

    let read_content = fs::read(&file_path).unwrap();
    assert_eq!(read_content, content);

    let metadata = fs::metadata(&file_path).unwrap();
    let permissions = metadata.permissions();
    assert_eq!(permissions.mode() & 0o777, 0o644);
}

#[test]
fn test_create_dir_all() {
    let dir = tempdir().unwrap();
    let dir_path = dir.path().join("a/b/c");
    let manager = FileManager::new();

    manager
        .create_dir_all(&dir_path, Some(0o755), None, None)
        .unwrap();

    assert!(dir_path.exists());
    assert!(dir_path.is_dir());

    let metadata = fs::metadata(&dir_path).unwrap();
    let permissions = metadata.permissions();
    assert_eq!(permissions.mode() & 0o777, 0o755);
}

#[test]
fn test_create_symlink() {
    let dir = tempdir().unwrap();
    let original = dir.path().join("original.txt");
    let link = dir.path().join("link.txt");
    let manager = FileManager::new();

    fs::write(&original, b"original").unwrap();

    manager
        .create_symlink(&original, &link, None, None)
        .unwrap();

    assert!(link.exists());
    assert_eq!(fs::read_link(&link).unwrap(), original);
}

#[test]
fn test_circular_symlink() {
    let dir = tempdir().unwrap();
    let link = dir.path().join("circular_link");
    let original = dir.path().join("circular_link"); // self loop

    let manager = FileManager::new();

    let result = manager.create_symlink(&original, &link, None, None);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn test_circular_symlink_indirect() {
    let dir = tempdir().unwrap();
    let link1 = dir.path().join("link1");
    let link2 = dir.path().join("link2");

    let manager = FileManager::new();

    // link1 -> link2
    manager.create_symlink(&link2, &link1, None, None).unwrap();

    // link2 -> link1 should fail
    let result = manager.create_symlink(&link1, &link2, None, None);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}

#[test]
fn test_circular_symlink_relative() {
    let dir = tempdir().unwrap();
    let subdir1 = dir.path().join("sub1");
    let subdir2 = dir.path().join("sub2");
    fs::create_dir_all(&subdir1).unwrap();
    fs::create_dir_all(&subdir2).unwrap();

    let link1 = subdir1.join("link1");
    let link2 = subdir2.join("link2");

    let manager = FileManager::new();

    // link1 points to link2 relatively: ../sub2/link2
    manager
        .create_symlink(Path::new("../sub2/link2"), &link1, None, None)
        .unwrap();

    // link2 points to link1 relatively: ../sub1/link1. This creates a circle.
    let result = manager.create_symlink(Path::new("../sub1/link1"), &link2, None, None);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidInput);
}

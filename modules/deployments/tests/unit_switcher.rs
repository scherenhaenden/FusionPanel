use deployments::switcher::switch_symlink;
use std::fs;
use tempfile::tempdir;

#[test]
fn test_switch_symlink_atomic() {
    let dir = tempdir().unwrap();
    let base = dir.path();

    let release1 = base.join("release1");
    let release2 = base.join("release2");

    fs::create_dir(&release1).unwrap();
    fs::create_dir(&release2).unwrap();

    fs::write(release1.join("index.html"), "release 1").unwrap();
    fs::write(release2.join("index.html"), "release 2").unwrap();

    let current_symlink = base.join("current");

    // Initial switch (no existing symlink)
    switch_symlink(&current_symlink, &release1).unwrap();

    assert!(current_symlink.exists());
    assert_eq!(
        fs::read_to_string(current_symlink.join("index.html")).unwrap(),
        "release 1"
    );

    // The previous release target should still exist
    assert!(release1.exists());

    // Atomic switch (existing symlink)
    switch_symlink(&current_symlink, &release2).unwrap();

    assert!(current_symlink.exists());
    assert_eq!(
        fs::read_to_string(current_symlink.join("index.html")).unwrap(),
        "release 2"
    );

    // The previous release target should still exist (never removed)
    assert!(release1.exists());
    assert!(release2.exists());
}

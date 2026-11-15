#![forbid(unsafe_code)]

use administration::etckeeper_guard::{DriftPolicy, EtckeeperGuard};
use std::fs;
use std::process::Command;
use tempfile::tempdir;

fn setup_git_repo(dir: &std::path::Path) {
    Command::new("git")
        .current_dir(dir)
        .args(&["init"])
        .output()
        .expect("git init failed");
    Command::new("git")
        .current_dir(dir)
        .args(&["config", "user.name", "Test User"])
        .output()
        .expect("git config user.name failed");
    Command::new("git")
        .current_dir(dir)
        .args(&["config", "user.email", "test@example.com"])
        .output()
        .expect("git config user.email failed");
}

fn get_last_commit_message(dir: &std::path::Path) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(&["log", "-1", "--pretty=%B"])
        .output()
        .expect("git log failed");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn test_get_status_no_drift() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    let file_path = dir.path().join("test.txt");
    fs::write(&file_path, "test").unwrap();

    Command::new("git")
        .current_dir(dir.path())
        .args(&["add", "test.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(dir.path())
        .args(&["commit", "-m", "Initial commit"])
        .output()
        .unwrap();

    let guard = EtckeeperGuard::new(dir.path(), DriftPolicy::Strict);
    let status = guard.get_status().unwrap();

    assert!(!status.has_drift());
    assert!(status.modified.is_empty());
    assert!(status.added.is_empty());
    assert!(status.deleted.is_empty());
    assert!(status.untracked.is_empty());
}

#[test]
fn test_get_status_with_drift() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Initial commit
    fs::write(dir.path().join("existing.txt"), "v1").unwrap();
    Command::new("git")
        .current_dir(dir.path())
        .args(&["add", "existing.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(dir.path())
        .args(&["commit", "-m", "Initial commit"])
        .output()
        .unwrap();

    // Create drift
    fs::write(dir.path().join("existing.txt"), "v2").unwrap(); // Modified
    fs::write(dir.path().join("untracked.txt"), "new").unwrap(); // Untracked
    fs::write(dir.path().join("added.txt"), "add").unwrap(); // Added
    Command::new("git")
        .current_dir(dir.path())
        .args(&["add", "added.txt"])
        .output()
        .unwrap();

    let guard = EtckeeperGuard::new(dir.path(), DriftPolicy::Strict);
    let status = guard.get_status().unwrap();

    assert!(status.has_drift());
    assert_eq!(status.modified, vec!["existing.txt".to_string()]);
    assert_eq!(status.added, vec!["added.txt".to_string()]);
    assert_eq!(status.untracked, vec!["untracked.txt".to_string()]);
    assert!(status.deleted.is_empty());
}

#[test]
fn test_policy_strict_aborts_on_drift() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    fs::write(dir.path().join("untracked.txt"), "new").unwrap();

    let guard = EtckeeperGuard::new(dir.path(), DriftPolicy::Strict);
    let result = guard.prepare_for_update("modifying nginx vhosts");

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Uncommitted drift detected: Found uncommitted changes in /etc"
    );
}

#[test]
fn test_policy_autocommit_resolves_drift() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Needs an initial commit to avoid strange behavior with empty repos
    fs::write(dir.path().join("init.txt"), "init").unwrap();
    Command::new("git")
        .current_dir(dir.path())
        .args(&["add", "init.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(dir.path())
        .args(&["commit", "-m", "Initial commit"])
        .output()
        .unwrap();

    fs::write(dir.path().join("untracked.txt"), "new").unwrap();

    let guard = EtckeeperGuard::new(dir.path(), DriftPolicy::AutoCommit);
    let result = guard.prepare_for_update("modifying nginx vhosts");

    assert!(result.is_ok());

    // AutoCommit will first commit the drift, then nothing for pre-update because we already did it?
    // Wait, prepare_for_update always commits "[FusionPanel] Pre-update: ..." if there is drift it commits twice or once?
    // Actually the logic says if drift, commit "Auto-commit...". Then it commits "Pre-update...".
    // Let's see what the last commit is. It should be Pre-update because git commit without changes fails but we handled it.
    // Wait, if AutoCommit committed the untracked file, then the repo is clean.
    // So the second commit for Pre-update will just fail to commit (nothing to commit) but return Ok(()).
    // So the last commit will be the "Auto-commit..." one, UNLESS we made a change before the second commit.
    // Actually, in prepare_for_update, the message says:
    // "We can just commit anything that might be there, though it should be clean now"
    // Wait, if `has_drift` is false initially, we commit "Pre-update...". If there's no drift, git commit does nothing and returns Ok(()).
    let last_msg = get_last_commit_message(dir.path());
    assert_eq!(last_msg, "[FusionPanel] Auto-commit: resolving drift before updates");
}

#[test]
fn test_post_update_commit() {
    let dir = tempdir().unwrap();
    setup_git_repo(dir.path());

    // Initial commit
    fs::write(dir.path().join("init.txt"), "init").unwrap();
    Command::new("git")
        .current_dir(dir.path())
        .args(&["add", "init.txt"])
        .output()
        .unwrap();
    Command::new("git")
        .current_dir(dir.path())
        .args(&["commit", "-m", "Initial commit"])
        .output()
        .unwrap();

    let guard = EtckeeperGuard::new(dir.path(), DriftPolicy::Strict);

    // Simulate panel making a change
    fs::write(dir.path().join("nginx.conf"), "server {}").unwrap();

    // Commit the change
    guard.post_update_commit("updated nginx.conf").unwrap();

    let last_msg = get_last_commit_message(dir.path());
    assert_eq!(last_msg, "[FusionPanel] Post-update: updated nginx.conf");

    // Ensure status is clean
    let status = guard.get_status().unwrap();
    assert!(!status.has_drift());
}

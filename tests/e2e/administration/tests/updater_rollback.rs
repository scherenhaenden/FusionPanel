#![forbid(unsafe_code)]

use release_integration::updater::Updater;
use std::{fs, os::unix::fs::PermissionsExt, path::Path};

fn write_executable(path: &Path, body: &[u8]) {
    fs::write(path, body).unwrap();
    let mut permissions = fs::metadata(path).unwrap().permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions).unwrap();
}

#[test]
fn updater_preflight_failure_preserves_live_binary_and_does_not_leave_marker() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("fusionpanel-api");
    let absent_staging_binary = temp.path().join("missing-update");
    write_executable(&target, b"#!/bin/sh\nprintf old\n");
    let updater = Updater::new(&target);

    assert!(updater.update(&absent_staging_binary).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"#!/bin/sh\nprintf old\n");
    assert!(!updater.check_recovery().unwrap());
    assert!(!temp.path().join("fusionpanel-api.backup").exists());
}

#[test]
fn updater_replaces_binary_and_cleans_recovery_state_after_success() {
    let temp = tempfile::tempdir().unwrap();
    let target = temp.path().join("fusionpanel-api");
    let staged = temp.path().join("fusionpanel-api.new");
    write_executable(&target, b"#!/bin/sh\nprintf old\n");
    write_executable(&staged, b"#!/bin/sh\nprintf new\n");
    let updater = Updater::new(&target);

    updater.update(&staged).unwrap();
    assert_eq!(fs::read(&target).unwrap(), b"#!/bin/sh\nprintf new\n");
    assert!(fs::metadata(&target).unwrap().permissions().mode() & 0o111 != 0);
    assert!(!updater.check_recovery().unwrap());
    assert!(!temp.path().join("fusionpanel-api.backup").exists());
}

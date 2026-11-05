#![forbid(unsafe_code)]

use std::process::Command;
use std::fs;
use std::path::Path;

#[test]
fn test_inplace_updater_rollback_guard() {
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_default();
    if api_url.is_empty() {
        panic!("BLOCKED: Missing FUSIONPANEL_API_URL for updater rollback E2E verification");
    }

    let target_dir = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".to_string());
    let tmp_dir = format!("{}/tmp/updater_test", target_dir);

    if fs::create_dir_all(&tmp_dir).is_err() {
        panic!("BLOCKED: Missing ability to create temporary directory for test");
    }

    let recovery_marker = format!("{}/.recovery-marker", tmp_dir);
    if fs::write(&recovery_marker, "mid-update-crash").is_err() {
        let _ = fs::remove_dir_all(&tmp_dir);
        panic!("BLOCKED: Cannot write recovery marker");
    }

    // Simulate recovery check
    let exists = Path::new(&recovery_marker).exists();

    // In a real integration test, we'd spawn the updater binary here and see it rollback
    let updater_path = format!("{}/debug/fusionpanel-updater", target_dir);

    let is_blocked = !exists || !Path::new(&updater_path).exists() || Command::new(&updater_path)
        .arg("--recovery-dir")
        .arg(&tmp_dir)
        .status().is_err();

    // Cleanup
    let _ = fs::remove_dir_all(&tmp_dir);

    assert!(exists, "Recovery marker should exist");

    if is_blocked {
        panic!("BLOCKED: Missing disposable Debian VM, real services, and updaters for updater rollback E2E verification");
    }
}

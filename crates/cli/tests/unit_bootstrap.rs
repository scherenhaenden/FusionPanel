#![forbid(unsafe_code)]

use std::process::Command;
use tempfile::tempdir;
use identity::SqliteIdentityStore;

#[test]
fn test_bootstrap_operator() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("fusionpanel.db");

    let status = Command::new("cargo")
        .args([
            "run",
            "--bin",
            "fusionpanel-cli",
            "--",
            "bootstrap",
            "operator",
            "--database-url",
            db_path.to_str().unwrap(),
            "--username",
            "testadmin",
        ])
        .status()
        .expect("Failed to execute command");

    assert!(status.success());

    // Verify DB
    let store = SqliteIdentityStore::open(&db_path).unwrap();
    let user = store.get_user_by_username("testadmin").unwrap().unwrap();
    assert_eq!(user.username(), "testadmin");

    let bindings = store.get_user_role_bindings(user.id().clone()).unwrap();
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].role.role_type, identity::RoleType::SuperAdmin);
}

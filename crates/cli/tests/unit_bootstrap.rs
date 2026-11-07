#![forbid(unsafe_code)]

use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn test_bootstrap_operator_success() {
    let db_file = NamedTempFile::new().expect("failed to create temp file");
    let db_path = db_file.path().to_str().unwrap();

    let cargo_bin = env!("CARGO_BIN_EXE_fusionpanel-cli");
    let output = Command::new(cargo_bin)
        .arg("bootstrap")
        .arg("operator")
        .arg("--database")
        .arg(db_path)
        .arg("--username")
        .arg("admin")
        .arg("--email")
        .arg("admin@example.com")
        .arg("--generate-password")
        .output()
        .expect("failed to execute fusionpanel-cli");

    assert!(output.status.success(), "bootstrap operator failed");
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("Operator created successfully."));
    assert!(stdout.contains("Username: admin"));
    assert!(stdout.contains("Email: admin@example.com"));
    assert!(stdout.contains("Generated Password:"));

    // Verify it in the sqlite database
    let store = identity::storage::SqliteIdentityStore::open(db_path)
        .expect("failed to open sqlite database");
    let user = store
        .get_user_by_username("admin")
        .expect("failed to execute query")
        .expect("user not found");

    assert_eq!(user.email(), "admin@example.com");

    let bindings = store
        .get_user_role_bindings(user.id())
        .expect("failed to get bindings");
    assert_eq!(bindings.len(), 1);
    assert_eq!(bindings[0].role.name, "SuperAdmin");

    match bindings[0].scope {
        identity::rbac::Scope::Tenant { tenant_id } => {
            // scope should be tenant scope with a valid UUID format
            assert_eq!(tenant_id.to_string().len(), 36);
        }
        _ => panic!("Expected scope to be Tenant"),
    }
}

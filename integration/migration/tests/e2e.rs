#![forbid(unsafe_code)]

use integration_migration::executor::{MigrationExecutor, MigrationState};
use migration::inventory::{InventoryItem, InventoryItemType, MigrationInventory};
use std::fs;
use tempfile::tempdir_in;

fn setup_temp_dir() -> tempfile::TempDir {
    let target_dir = std::path::PathBuf::from("../../target/tmp");
    fs::create_dir_all(&target_dir).unwrap();
    tempdir_in(&target_dir).unwrap()
}

fn create_valid_inventory() -> MigrationInventory {
    let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
    inventory.items.push(InventoryItem {
        id: "site_1".to_string(),
        item_type: InventoryItemType::Site,
        name: "example.com".to_string(),
        version: Some("8.1".to_string()),
        metadata: std::collections::HashMap::new(),
        associations: vec!["db_1".to_string()],
        ownership: Some("admin_user".to_string()),
        aliases: vec!["www.example.com".to_string()],
        schedules: vec!["daily_backup".to_string()],
        variants: vec!["php81".to_string()],
        user_mappings: std::collections::HashMap::new(),
    });
    inventory.items.push(InventoryItem {
        id: "db_1".to_string(),
        item_type: InventoryItemType::Database,
        name: "example_db".to_string(),
        version: None,
        metadata: std::collections::HashMap::new(),
        associations: vec!["site_1".to_string()],
        ownership: Some("admin_user".to_string()),
        aliases: Vec::new(),
        schedules: Vec::new(),
        variants: Vec::new(),
        user_mappings: std::collections::HashMap::new(),
    });
    inventory
}

#[test]
fn test_successful_migration_apply() {
    let work_dir = setup_temp_dir();
    let mut executor = MigrationExecutor::new(work_dir.path().to_path_buf());

    let inventory = create_valid_inventory();

    executor.execute(&inventory).unwrap();
    assert_eq!(*executor.get_state(), MigrationState::Cutover);
    assert!(!work_dir.path().join(".recovery-marker").exists());
}

#[test]
fn test_interrupted_apply_restart() {
    let work_dir = setup_temp_dir();
    let work_path = work_dir.path().to_path_buf();

    fs::write(work_path.join(".recovery-marker"), "checkpoint").unwrap();

    let mut executor = MigrationExecutor::new(work_path.clone());
    let inventory = create_valid_inventory();

    executor.execute(&inventory).unwrap();
    assert_eq!(*executor.get_state(), MigrationState::Cutover);
    assert!(!work_path.join(".recovery-marker").exists());

    executor.execute(&inventory).unwrap();
    assert_eq!(*executor.get_state(), MigrationState::Cutover);
    assert!(!work_path.join(".recovery-marker").exists());
}

#[test]
fn test_conflicts_and_missing_support() {
    let work_dir = setup_temp_dir();
    let mut executor = MigrationExecutor::new(work_dir.path().to_path_buf());

    let mut inventory = create_valid_inventory();
    // Add conflicting user
    inventory.items.push(InventoryItem {
        id: "user_root".to_string(),
        item_type: InventoryItemType::User,
        name: "root".to_string(),
        version: None,
        metadata: std::collections::HashMap::new(),
        associations: Vec::new(),
        ownership: None,
        aliases: Vec::new(),
        schedules: Vec::new(),
        variants: Vec::new(),
        user_mappings: std::collections::HashMap::new(),
    });

    let result = executor.execute(&inventory);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Conflict"));
    assert_eq!(*executor.get_state(), MigrationState::Failed);
}

#[test]
fn test_rollback() {
    let work_dir = setup_temp_dir();
    let work_path = work_dir.path().to_path_buf();
    let mut executor = MigrationExecutor::new(work_path.clone());

    fs::write(work_path.join(".recovery-marker"), "checkpoint").unwrap();

    executor.rollback().unwrap();

    assert_eq!(*executor.get_state(), MigrationState::RolledBack);
    assert!(!work_path.join(".recovery-marker").exists());
}

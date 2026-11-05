#![allow(deprecated)]
use migration::inventory::{MigrationInventory, InventoryItem, InventoryItemType};
use integration_migration::executor::{MigrationExecutor, MigrationState};
use tempfile::tempdir_in;
use std::fs;

fn setup_temp_dir() -> std::path::PathBuf {
    let target_dir = std::path::PathBuf::from("../../target/tmp");
    fs::create_dir_all(&target_dir).unwrap();
    let dir = tempdir_in(&target_dir).unwrap();
    dir.into_path()
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
    let mut executor = MigrationExecutor::new(work_dir.clone());

    let inventory = create_valid_inventory();

    let result = executor.execute(&inventory);
    assert!(result.is_ok(), "Execution should succeed");

    assert_eq!(*executor.get_state(), MigrationState::Cutover);

    // Check that marker is cleaned up
    assert!(!work_dir.join(".recovery-marker").exists());
}

#[test]
fn test_interrupted_apply_restart() {
    let work_dir = setup_temp_dir();

    // Simulate an interrupted state by manually creating the marker
    fs::write(work_dir.join(".recovery-marker"), "checkpoint").unwrap();

    let mut executor = MigrationExecutor::new(work_dir.clone());
    let inventory = create_valid_inventory();

    // First execution should detect the marker, rollback, and fail with "Recovered"
    let result = executor.execute(&inventory);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Recovered from previous crash");
    assert_eq!(*executor.get_state(), MigrationState::RolledBack);

    // Check that marker is cleaned up by rollback
    assert!(!work_dir.join(".recovery-marker").exists());

    // Second execution should succeed
    let result2 = executor.execute(&inventory);
    assert!(result2.is_ok());
    assert_eq!(*executor.get_state(), MigrationState::Cutover);
}

#[test]
fn test_conflicts_and_missing_support() {
    let work_dir = setup_temp_dir();
    let mut executor = MigrationExecutor::new(work_dir.clone());

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
    let mut executor = MigrationExecutor::new(work_dir.clone());

    fs::write(work_dir.join(".recovery-marker"), "checkpoint").unwrap();

    executor.rollback().unwrap();

    assert_eq!(*executor.get_state(), MigrationState::RolledBack);
    assert!(!work_dir.join(".recovery-marker").exists());
}

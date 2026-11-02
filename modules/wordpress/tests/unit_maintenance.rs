use std::fs;
use tempfile::tempdir;
use wordpress::maintenance::Coordinator;

#[test]
fn test_maintenance_mode_toggle() {
    let dir = tempdir().unwrap();
    let coordinator = Coordinator::new(dir.path());

    // Should be initially disabled
    assert!(!coordinator.is_maintenance_mode_enabled());

    // Enable it
    coordinator.set_maintenance_mode(true).unwrap();
    assert!(coordinator.is_maintenance_mode_enabled());
    let maintenance_file = dir.path().join(".maintenance");
    assert!(maintenance_file.exists());
    let content = fs::read_to_string(&maintenance_file).unwrap();
    assert!(content.contains("$upgrading = time();"));

    // Disable it
    coordinator.set_maintenance_mode(false).unwrap();
    assert!(!coordinator.is_maintenance_mode_enabled());
    assert!(!maintenance_file.exists());
}

#[test]
fn test_offload_system_cron_no_config() {
    let dir = tempdir().unwrap();
    let coordinator = Coordinator::new(dir.path());

    let result = coordinator.offload_system_cron(true);
    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err().to_string(),
        "Configuration Error: wp-config.php not found"
    );
}

#[test]
fn test_offload_system_cron_insert() {
    let dir = tempdir().unwrap();
    let coordinator = Coordinator::new(dir.path());
    let config_path = dir.path().join("wp-config.php");

    // Write dummy config
    let initial_content = "<?php\n// Some config\n/* That's all, stop editing! */\n";
    fs::write(&config_path, initial_content).unwrap();

    // Disable wp-cron (offload to system cron)
    coordinator.offload_system_cron(true).unwrap();

    let new_content = fs::read_to_string(&config_path).unwrap();
    assert!(new_content.contains("define( 'DISABLE_WP_CRON', true );"));

    // Test toggle off
    coordinator.offload_system_cron(false).unwrap();
    let new_content2 = fs::read_to_string(&config_path).unwrap();
    assert!(new_content2.contains("define( 'DISABLE_WP_CRON', false );"));
}

#[test]
fn test_offload_system_cron_update_existing() {
    let dir = tempdir().unwrap();
    let coordinator = Coordinator::new(dir.path());
    let config_path = dir.path().join("wp-config.php");

    // Write dummy config with existing flag
    let initial_content = "<?php\ndefine( 'DISABLE_WP_CRON', false );\n";
    fs::write(&config_path, initial_content).unwrap();

    // Set to true
    coordinator.offload_system_cron(true).unwrap();
    let new_content = fs::read_to_string(&config_path).unwrap();
    assert!(new_content.contains("define( 'DISABLE_WP_CRON', true );"));
    assert!(!new_content.contains("false")); // The old one is replaced
}

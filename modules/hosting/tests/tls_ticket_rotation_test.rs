#![forbid(unsafe_code)]

use hosting::tls_ticket_rotation::{StekManager, KeyState};
use std::thread;
use std::time::Duration;
use tempfile::tempdir;
use std::fs;

#[test]
fn test_key_generation_and_rotation() {
    let mut manager = StekManager::new(43200, 172800); // 12h rot, 48h ttl

    manager.maybe_rotate().unwrap();
    assert_eq!(manager.get_valid_keys().len(), 1);

    let primary = manager.get_primary().unwrap();
    assert_eq!(primary.state, KeyState::Primary);

    manager.rotate_keys().unwrap();
    assert_eq!(manager.get_valid_keys().len(), 2);

    let keys = manager.get_valid_keys();
    assert_eq!(keys[0].state, KeyState::Primary);
    assert_eq!(keys[1].state, KeyState::Secondary);
}

#[test]
fn test_maybe_rotate() {
    let mut manager = StekManager::new(1, 172800); // 1s rot

    manager.maybe_rotate().unwrap(); // primary generated
    assert_eq!(manager.get_valid_keys().len(), 1);

    manager.maybe_rotate().unwrap(); // no rotation since time not passed
    assert_eq!(manager.get_valid_keys().len(), 1);

    thread::sleep(Duration::from_secs(2));

    manager.maybe_rotate().unwrap(); // rotation
    assert_eq!(manager.get_valid_keys().len(), 2);
}

#[test]
fn test_expired_key_pruning() {
    let mut manager = StekManager::new(43200, 1); // 1s ttl
    manager.rotate_keys().unwrap(); // primary generated

    thread::sleep(Duration::from_secs(2));

    // Since TTL is 1, it's expired immediately for getters
    assert_eq!(manager.get_valid_keys().len(), 0);

    // on next rotation it should be expired
    manager.rotate_keys().unwrap();

    // Check that one is expired and one is primary, so length is 1 for valid
    assert_eq!(manager.get_valid_keys().len(), 1);

    // But prune will remove expired entirely from memory
    manager.prune_expired();
    assert_eq!(manager.get_valid_keys().len(), 1);
}

#[test]
fn test_generate_configs() {
    let mut manager = StekManager::new(43200, 172800);
    manager.rotate_keys().unwrap();
    manager.rotate_keys().unwrap();

    let config = manager.generate_nginx_config("/etc/nginx/tickets");
    assert_eq!(config.lines().count(), 2);
    for line in config.lines() {
        assert!(line.starts_with("ssl_session_ticket_key /etc/nginx/tickets/"));
        assert!(line.ends_with(".key;"));
    }

    let config_apache = manager.generate_apache_config("/etc/apache2/tickets");
    assert_eq!(config_apache.lines().count(), 2);
    for line in config_apache.lines() {
        assert!(line.starts_with("SSLSessionTicketKeyFile /etc/apache2/tickets/"));
        assert!(line.ends_with(".key"));
    }
}

#[test]
fn test_lookup_key() {
    let mut manager = StekManager::new(43200, 172800);
    manager.rotate_keys().unwrap();

    let primary = manager.get_primary().unwrap();
    let name = primary.get_name();

    let looked_up = manager.lookup_key(name).unwrap();
    assert_eq!(looked_up.data, primary.data);
}

#[test]
fn test_save_keys_to_disk() {
    let mut manager = StekManager::new(43200, 172800);
    manager.rotate_keys().unwrap();

    let temp_dir = tempdir().unwrap();
    manager.save_keys_to_disk(temp_dir.path()).unwrap();

    let keys = manager.get_valid_keys();
    let file_path = temp_dir.path().join(format!("{:x}.key", keys[0].created_at.timestamp_micros()));
    let data = fs::read(file_path).unwrap();

    assert_eq!(data, keys[0].data);
}

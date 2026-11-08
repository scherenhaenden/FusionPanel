#![forbid(unsafe_code)]

use security::authorized_keys::{SshKeyManager, SshKeyError};
use std::fs;
use std::os::unix::fs::PermissionsExt;

#[test]
fn test_valid_ssh_key_parsing() {
    let key_data = "AAAAC3NzaC1lZDI1NTE5AAAAIM13y27G9zN0hL9p/qZ+dG10X3M6uT5OqXzTz7PzE6mY";
    let line = format!("ssh-ed25519 {} user@example.com", key_data);
    let manager = SshKeyManager::parse(&line).unwrap();
    let keys = manager.get_keys();
    assert_eq!(keys.len(), 1);
    
    let key = keys[0];
    assert_eq!(key.key_type, "ssh-ed25519");
    assert_eq!(key.key_data, key_data);
    assert_eq!(key.comment.as_deref(), Some("user@example.com"));
    
    // Check fingerprint logic (starts with SHA256:)
    assert!(key.fingerprint.starts_with("SHA256:"));
}

#[test]
fn test_invalid_key_type() {
    let line = "ssh-unknown AAAAB3NzaC... user@example";
    let key = SshKeyManager::parse_key(line);
    assert!(matches!(key, Err(SshKeyError::UnsupportedKeyType)));
}

#[test]
fn test_corrupted_base64() {
    let line = "ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIM13y27G9zN0hL9p/qZ+dG10X3M6uT5OqXzTz7PzE6mY! user@example.com";
    let key = SshKeyManager::parse_key(line);
    assert!(matches!(key, Err(SshKeyError::Base64DecodeError)));
}

#[test]
fn test_duplicate_detection() {
    let key_data = "AAAAC3NzaC1lZDI1NTE5AAAAIM13y27G9zN0hL9p/qZ+dG10X3M6uT5OqXzTz7PzE6mY";
    let line1 = format!("ssh-ed25519 {} user1@example.com", key_data);
    let line2 = format!("ssh-ed25519 {} user2@example.com", key_data); // Same key, different comment
    
    let content = format!("{}\n{}", line1, line2);
    let manager = SshKeyManager::parse(&content).unwrap();
    
    // Should deduplicate by fingerprint
    let keys = manager.get_keys();
    assert_eq!(keys.len(), 1);
}

#[test]
fn test_comments_handling_and_serialization() {
    let key_data = "AAAAC3NzaC1lZDI1NTE5AAAAIM13y27G9zN0hL9p/qZ+dG10X3M6uT5OqXzTz7PzE6mY";
    let line = format!("ssh-ed25519 {}", key_data); // No comment
    
    let manager = SshKeyManager::parse(&line).unwrap();
    let keys = manager.get_keys();
    assert_eq!(keys.len(), 1);
    assert_eq!(keys[0].comment, None);
    
    let serialized = manager.serialize();
    assert_eq!(serialized, format!("ssh-ed25519 {}\n", key_data));
}

#[test]
fn test_add_remove_key() {
    let mut manager = SshKeyManager::new();
    let key_data = "AAAAC3NzaC1lZDI1NTE5AAAAIM13y27G9zN0hL9p/qZ+dG10X3M6uT5OqXzTz7PzE6mY";
    let line = format!("ssh-ed25519 {} test", key_data);
    
    let key = SshKeyManager::parse_key(&line).unwrap();
    let fp = key.fingerprint.clone();
    
    assert!(manager.add_key(key.clone()));
    assert!(!manager.add_key(key)); // Adding duplicate returns false
    
    assert_eq!(manager.get_keys().len(), 1);
    
    assert!(manager.remove_key_by_fingerprint(&fp));
    assert_eq!(manager.get_keys().len(), 0);
    assert!(!manager.remove_key_by_fingerprint(&fp)); // Already removed
}

#[test]
fn test_write_to_file_permissions() {
    let key_data = "AAAAC3NzaC1lZDI1NTE5AAAAIM13y27G9zN0hL9p/qZ+dG10X3M6uT5OqXzTz7PzE6mY";
    let line = format!("ssh-ed25519 {}", key_data);
    let manager = SshKeyManager::parse(&line).unwrap();
    
    let temp_dir = std::env::temp_dir();
    let file_path = temp_dir.join("test_authorized_keys");
    
    manager.write_to_file(&file_path).unwrap();
    
    let metadata = fs::metadata(&file_path).unwrap();
    let perms = metadata.permissions();
    assert_eq!(perms.mode() & 0o777, 0o600);
    
    let content = fs::read_to_string(&file_path).unwrap();
    assert_eq!(content, format!("ssh-ed25519 {}\n", key_data));
    
    fs::remove_file(file_path).unwrap();
}

#[test]
fn test_ignore_empty_and_comments() {
    let content = "
# This is a comment

ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIM13y27G9zN0hL9p/qZ+dG10X3M6uT5OqXzTz7PzE6mY
  
# Another comment
";
    let manager = SshKeyManager::parse(content).unwrap();
    assert_eq!(manager.get_keys().len(), 1);
}

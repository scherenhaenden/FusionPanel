#![forbid(unsafe_code)]

use secrets::envelope_encryption::{Dek, EnvelopeKeyManager, Kek};
use secrets::key_rotation::{KeyRotationPlanner, RotationStatus};

#[test]
fn test_rotation_initialization() {
    let planner = KeyRotationPlanner::new(1);
    assert_eq!(planner.current_version, 1);
    assert_eq!(planner.target_version, 2);
    assert_eq!(planner.status, RotationStatus::Pending);
}

#[test]
fn test_rotation_flow() {
    let mut planner = KeyRotationPlanner::new(1);
    assert!(planner.begin_rotation().is_ok());
    assert_eq!(planner.status, RotationStatus::InProgress);

    let old_kek = Kek::generate();
    let new_kek = Kek::generate();
    let dek = Dek::generate();
    let wrapped_dek = EnvelopeKeyManager::wrap_dek(&old_kek, &dek).unwrap();

    let new_wrapped_dek = planner.reencrypt_wrapped_dek(&old_kek, &new_kek, &wrapped_dek);
    assert!(new_wrapped_dek.is_ok());

    let unwrapped_dek = EnvelopeKeyManager::unwrap_dek(&new_kek, &new_wrapped_dek.unwrap()).unwrap();
    assert_eq!(dek.as_bytes(), unwrapped_dek.as_bytes());

    assert!(planner.mark_completed().is_ok());
    assert_eq!(planner.status, RotationStatus::Completed);
}

#[test]
fn test_rotation_reencryption_requires_in_progress() {
    let planner = KeyRotationPlanner::new(1);
    let old_kek = Kek::generate();
    let new_kek = Kek::generate();
    let dek = Dek::generate();
    let wrapped_dek = EnvelopeKeyManager::wrap_dek(&old_kek, &dek).unwrap();

    let result = planner.reencrypt_wrapped_dek(&old_kek, &new_kek, &wrapped_dek);
    assert!(result.is_err());
}

#[test]
fn test_retirement_gate() {
    let mut planner = KeyRotationPlanner::new(2);

    // When Pending, active_version = 2
    assert!(planner.enforce_retirement_gate(1).is_err());
    assert!(planner.enforce_retirement_gate(2).is_ok());

    planner.begin_rotation().unwrap();
    // When InProgress, active_version = 2
    assert!(planner.enforce_retirement_gate(1).is_err());
    assert!(planner.enforce_retirement_gate(2).is_ok());

    planner.mark_completed().unwrap();
    // When Completed, active_version = target_version = 3
    assert!(planner.enforce_retirement_gate(2).is_err());
    assert!(planner.enforce_retirement_gate(3).is_ok());
}

#[test]
fn test_mark_failed() {
    let mut planner = KeyRotationPlanner::new(1);
    planner.begin_rotation().unwrap();
    planner.mark_failed("Network error".to_string());
    assert_eq!(planner.status, RotationStatus::Failed("Network error".to_string()));
}

#![forbid(unsafe_code)]

use security::{WebAuthnStore, WebAuthnError};
use std::time::{Duration, SystemTime};

#[test]
fn test_generate_and_verify_challenge() {
    let mut store = WebAuthnStore::new();
    let challenge = store.generate_challenge().expect("Failed to generate challenge");

    assert_eq!(challenge.len(), 32);

    // Verify successfully
    let result = store.verify_challenge(&challenge);
    assert_eq!(result, Ok(()));

    // Verify again fails (single use)
    let result2 = store.verify_challenge(&challenge);
    assert_eq!(result2, Err(WebAuthnError::InvalidChallenge));
}

#[test]
fn test_verify_expired_challenge() {
    let mut store = WebAuthnStore::new();
    let challenge = store.generate_challenge().expect("Failed to generate challenge");

    // Fast-forward time
    let future_time = SystemTime::now() + Duration::from_secs(61);

    // Verify at future time should fail
    let result = store.verify_challenge_at(&challenge, future_time);
    assert_eq!(result, Err(WebAuthnError::InvalidChallenge));
}

#[test]
fn test_cleanup_expired_challenges() {
    let mut store = WebAuthnStore::new();
    let challenge = store.generate_challenge().expect("Failed to generate challenge");

    // Immediately it should be there, but we want to simulate expiration.
    // We can't directly mutate the internal challenge time without mocking or time travel,
    // but we can verify that before expiration, cleanup does not remove it.
    store.cleanup_expired_challenges();
    assert_eq!(store.verify_challenge(&challenge), Ok(()));
}

#[test]
fn test_register_and_get_credential() {
    let mut store = WebAuthnStore::new();
    let id = vec![1, 2, 3];
    let public_key = vec![4, 5, 6];
    let initial_counter = 10;

    let res = store.register_credential(id.clone(), public_key.clone(), initial_counter);
    assert_eq!(res, Ok(()));

    // Try to register again
    let res2 = store.register_credential(id.clone(), public_key.clone(), initial_counter);
    assert_eq!(res2, Err(WebAuthnError::CredentialAlreadyRegistered));

    // Get credential
    let cred = store.get_credential(&id).expect("Credential should exist");
    assert_eq!(cred.id, id);
    assert_eq!(cred.public_key, public_key);
    assert_eq!(cred.signature_counter, initial_counter);
}

#[test]
fn test_update_signature_counter() {
    let mut store = WebAuthnStore::new();
    let id = vec![1, 2, 3];
    let public_key = vec![4, 5, 6];

    store.register_credential(id.clone(), public_key.clone(), 10).unwrap();

    // Increasing counter succeeds
    assert_eq!(store.update_signature_counter(&id, 11), Ok(()));
    assert_eq!(store.get_credential(&id).unwrap().signature_counter, 11);

    // Same counter fails
    assert_eq!(store.update_signature_counter(&id, 11), Err(WebAuthnError::ReplayDetected));

    // Lower counter fails
    assert_eq!(store.update_signature_counter(&id, 5), Err(WebAuthnError::ReplayDetected));

    // Unknown credential
    assert_eq!(store.update_signature_counter(&vec![9, 9, 9], 15), Err(WebAuthnError::CredentialNotFound));
}

#[test]
fn test_zero_counter_authenticator() {
    let mut store = WebAuthnStore::new();
    let id = vec![7, 8, 9];
    let public_key = vec![10, 11, 12];

    // Register with 0 initial counter
    store.register_credential(id.clone(), public_key.clone(), 0).unwrap();

    // Same counter 0 succeeds for authenticators not supporting counters
    assert_eq!(store.update_signature_counter(&id, 0), Ok(()));

    // If it later upgrades to supporting counters
    assert_eq!(store.update_signature_counter(&id, 1), Ok(()));

    // Once > 0, 0 is no longer allowed
    assert_eq!(store.update_signature_counter(&id, 0), Err(WebAuthnError::ReplayDetected));
}

#[test]
fn test_challenge_uniqueness() {
    let mut store = WebAuthnStore::new();
    let c1 = store.generate_challenge().unwrap();
    let c2 = store.generate_challenge().unwrap();

    assert_ne!(c1, c2, "Challenges should be unique");
}

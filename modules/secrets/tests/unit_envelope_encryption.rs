#![forbid(unsafe_code)]

use secrets::envelope_encryption::{Dek, EnvelopeKeyManager, Kek};

#[test]
fn test_dek_generation_and_sizes() {
    let dek = Dek::generate();
    assert_eq!(dek.as_bytes().len(), 32);

    let kek = Kek::generate();
    // Wrap to ensure lengths don't break during encryption
    let wrapped = EnvelopeKeyManager::wrap_dek(&kek, &dek).expect("wrap failed");
    assert_eq!(wrapped.nonce.len(), 12);
    // Ciphertext size depends on MAC tag size (16) + plaintext (32) = 48
    assert_eq!(wrapped.ciphertext.len(), 48);
}

#[test]
fn test_wrap_and_unwrap_dek() {
    let kek = Kek::generate();
    let dek = Dek::generate();

    let wrapped = EnvelopeKeyManager::wrap_dek(&kek, &dek).expect("Failed to wrap DEK");
    let unwrapped_dek = EnvelopeKeyManager::unwrap_dek(&kek, &wrapped).expect("Failed to unwrap DEK");

    assert_eq!(dek.as_bytes(), unwrapped_dek.as_bytes());
}

#[test]
fn test_kek_rotation() {
    let old_kek = Kek::generate();
    let new_kek = Kek::generate();
    let dek = Dek::generate();

    let wrapped_with_old = EnvelopeKeyManager::wrap_dek(&old_kek, &dek).expect("Failed to wrap DEK with old KEK");

    // Rotate
    let wrapped_with_new = EnvelopeKeyManager::rotate_kek(&old_kek, &new_kek, &wrapped_with_old)
        .expect("Failed to rotate KEK");

    // Ensure we can unwrap with new KEK and the value is correct
    let unwrapped_new = EnvelopeKeyManager::unwrap_dek(&new_kek, &wrapped_with_new).expect("Failed to unwrap rotated DEK");

    assert_eq!(dek.as_bytes(), unwrapped_new.as_bytes());

    // Ensure old KEK cannot unwrap the new wrapped DEK
    assert!(EnvelopeKeyManager::unwrap_dek(&old_kek, &wrapped_with_new).is_err());
}

#[test]
fn test_wrong_key_rejection() {
    let kek1 = Kek::generate();
    let kek2 = Kek::generate();
    let dek = Dek::generate();

    let wrapped = EnvelopeKeyManager::wrap_dek(&kek1, &dek).expect("Failed to wrap DEK");

    // Try to unwrap with a different KEK
    assert!(EnvelopeKeyManager::unwrap_dek(&kek2, &wrapped).is_err());
}

#[test]
fn test_ciphertext_integrity() {
    let kek = Kek::generate();
    let dek = Dek::generate();

    let mut wrapped = EnvelopeKeyManager::wrap_dek(&kek, &dek).expect("Failed to wrap DEK");

    // Mutate a byte in ciphertext
    wrapped.ciphertext[0] ^= 1;

    assert!(EnvelopeKeyManager::unwrap_dek(&kek, &wrapped).is_err());
}

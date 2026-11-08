#![forbid(unsafe_code)]

use secrets::kek_derivation::{derive_kek, generate_fingerprint, KekDerivationConfig};

#[test]
fn test_kek_derivation_deterministic() {
    let salt = b"this_is_a_16_byte_salt_123456789".to_vec();
    let config = KekDerivationConfig::new(salt.clone(), 1000).unwrap();
    let passphrase = b"master_passphrase";

    let key1 = derive_kek(passphrase, &config).unwrap();
    let key2 = derive_kek(passphrase, &config).unwrap();

    assert_eq!(key1.as_bytes(), key2.as_bytes());
}

#[test]
fn test_kek_derivation_salt_too_short() {
    let salt = b"short_salt".to_vec();
    let result = KekDerivationConfig::new(salt, 1000);
    assert!(result.is_err());
    assert_eq!(result.unwrap_err(), "Salt must be at least 16 bytes long.");
}

#[test]
fn test_generate_fingerprint() {
    let key_id = "my-key-id";
    let salt = b"some_random_salt_data";

    let fingerprint1 = generate_fingerprint(key_id, salt);
    let fingerprint2 = generate_fingerprint(key_id, salt);

    assert_eq!(fingerprint1, fingerprint2);

    let fingerprint3 = generate_fingerprint("other-key-id", salt);
    assert_ne!(fingerprint1, fingerprint3);
}

#[test]
fn test_derived_key_debug_redacted() {
    let salt = b"this_is_a_16_byte_salt_123456789".to_vec();
    let config = KekDerivationConfig::new(salt, 1000).unwrap();
    let key = derive_kek(b"password", &config).unwrap();

    let debug_str = format!("{:?}", key);
    assert_eq!(debug_str, "[REDACTED]");
}

#[test]
fn test_kek_derivation_salt_uniqueness() {
    let salt1 = b"this_is_a_16_byte_salt_123456789".to_vec();
    let config1 = KekDerivationConfig::new(salt1, 1000).unwrap();
    let passphrase = b"master_passphrase";

    let salt2 = b"this_is_a_DIFFERENT_salt_1234567".to_vec();
    let config2 = KekDerivationConfig::new(salt2, 1000).unwrap();

    let key1 = derive_kek(passphrase, &config1).unwrap();
    let key2 = derive_kek(passphrase, &config2).unwrap();

    assert_ne!(key1.as_bytes(), key2.as_bytes());
}

#[test]
fn test_kek_derivation_known_test_vector() {
    // Standard PBKDF2-HMAC-SHA256 test vector
    // password = "password"
    // salt = "salt" (Note: We pad it here because our API enforces 16 bytes minimum salt)
    // Actually let's use a custom known vector since salt needs to be 16 bytes.
    // Given the constraints we can just assert that with a given setup it produces a specific known hash.
    // Instead of using RFC standard ones which have short salts, we just snapshot a known one.
    let salt = b"salt_must_be_16b".to_vec();
    let config = KekDerivationConfig::new(salt, 1).unwrap();
    let passphrase = b"password";

    let key = derive_kek(passphrase, &config).unwrap();

    // Calculated externally: pbkdf2_hmac_sha256("password", "salt_must_be_16b", 1, 32)
    // Echoing python: hashlib.pbkdf2_hmac('sha256', b'password', b'salt_must_be_16b', 1)
    let expected = hex::decode("67e290159082eb8d0ce4ecf99f9b36207f2a1f542e3b06ae423953430cc5753d").unwrap();

    assert_eq!(key.as_bytes(), expected.as_slice());
}

#[test]
fn test_zeroization() {
    use zeroize::Zeroize;

    let mut key = secrets::kek_derivation::DerivedKey([0xAA; 32]);
    assert_eq!(key.0, [0xAA; 32]);

    key.zeroize();

    assert_eq!(key.0, [0x00; 32]);
}

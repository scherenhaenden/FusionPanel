#![forbid(unsafe_code)]

use pbkdf2::pbkdf2_hmac;
use sha2::{Digest, Sha256};
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Configuration for Key Encryption Key (KEK) derivation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KekDerivationConfig {
    /// Salt must be at least 16 bytes.
    pub salt: Vec<u8>,
    /// Number of iterations for PBKDF2.
    pub iterations: u32,
    /// Expected output key length in bytes. Only 32 (256-bit) is supported.
    pub output_len: usize,
}

impl KekDerivationConfig {
    /// Create a new derivation config. Returns an error if the salt is too short
    /// or if the output length is not 32.
    pub fn new(salt: Vec<u8>, iterations: u32) -> Result<Self, &'static str> {
        if salt.len() < 16 {
            return Err("Salt must be at least 16 bytes long.");
        }
        Ok(Self {
            salt,
            iterations,
            output_len: 32,
        })
    }
}

/// Derived KEK that automatically zeroizes its memory when dropped.
#[derive(Clone, Zeroize, ZeroizeOnDrop)]
pub struct DerivedKey(pub [u8; 32]);

impl DerivedKey {
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

impl std::fmt::Debug for DerivedKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("[REDACTED]")
    }
}

/// Derive a Key Encryption Key (KEK) from a passphrase and a configuration.
pub fn derive_kek(passphrase: &[u8], config: &KekDerivationConfig) -> Result<DerivedKey, &'static str> {
    if config.output_len != 32 {
        return Err("Only 256-bit (32 bytes) output length is supported.");
    }

    let mut key = [0u8; 32];
    pbkdf2_hmac::<Sha256>(passphrase, &config.salt, config.iterations, &mut key);

    Ok(DerivedKey(key))
}

/// Generate a fingerprint (identifier) for a KEK using SHA-256 over the key ID and the salt.
pub fn generate_fingerprint(key_id: &str, salt: &[u8]) -> [u8; 32] {
    let mut hasher = Sha256::new();
    hasher.update(key_id.as_bytes());
    hasher.update(salt);

    let mut out = [0u8; 32];
    out.copy_from_slice(&hasher.finalize());
    out
}

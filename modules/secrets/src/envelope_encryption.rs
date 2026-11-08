#![forbid(unsafe_code)]

use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Key, Nonce,
};
use rand::{thread_rng, RngCore};
use std::fmt;
use zeroize::Zeroize;

#[derive(Debug)]
pub enum Error {
    EncryptionFailed,
    DecryptionFailed,
    InvalidKeyLength,
}

impl std::error::Error for Error {}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::EncryptionFailed => write!(f, "encryption failed"),
            Error::DecryptionFailed => write!(f, "decryption failed"),
            Error::InvalidKeyLength => write!(f, "invalid key length"),
        }
    }
}

pub struct Kek {
    key: [u8; 32],
}

impl Kek {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        thread_rng().fill_bytes(&mut key);
        Self { key }
    }
}

impl Drop for Kek {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl fmt::Debug for Kek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Kek([REDACTED])")
    }
}

pub struct Dek {
    key: [u8; 32],
}

impl Dek {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn generate() -> Self {
        let mut key = [0u8; 32];
        thread_rng().fill_bytes(&mut key);
        Self { key }
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.key
    }
}

impl Drop for Dek {
    fn drop(&mut self) {
        self.key.zeroize();
    }
}

impl fmt::Debug for Dek {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Dek([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WrappedDek {
    pub nonce: [u8; 12],
    pub ciphertext: Vec<u8>,
}

pub struct EnvelopeKeyManager;

impl EnvelopeKeyManager {
    /// Wraps a DEK using the provided KEK.
    pub fn wrap_dek(kek: &Kek, dek: &Dek) -> Result<WrappedDek, Error> {
        let key = Key::<Aes256Gcm>::from_slice(&kek.key);
        let cipher = Aes256Gcm::new(key);

        let mut nonce_bytes = [0u8; 12];
        thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, dek.key.as_ref())
            .map_err(|_| Error::EncryptionFailed)?;

        Ok(WrappedDek {
            nonce: nonce_bytes,
            ciphertext,
        })
    }

    /// Unwraps a wrapped DEK using the provided KEK.
    pub fn unwrap_dek(kek: &Kek, wrapped_dek: &WrappedDek) -> Result<Dek, Error> {
        let key = Key::<Aes256Gcm>::from_slice(&kek.key);
        let cipher = Aes256Gcm::new(key);
        let nonce = Nonce::from_slice(&wrapped_dek.nonce);

        let mut plaintext = cipher
            .decrypt(nonce, wrapped_dek.ciphertext.as_ref())
            .map_err(|_| Error::DecryptionFailed)?;

        if plaintext.len() != 32 {
            plaintext.zeroize();
            return Err(Error::InvalidKeyLength);
        }

        let mut dek_key = [0u8; 32];
        dek_key.copy_from_slice(&plaintext);

        // Zeroize plaintext buffer
        plaintext.zeroize();

        Ok(Dek::new(dek_key))
    }

    /// Re-wraps an existing wrapped DEK with a new KEK, without needing to
    /// decrypt the underlying secret payloads (because the DEK remains unchanged).
    pub fn rotate_kek(
        old_kek: &Kek,
        new_kek: &Kek,
        wrapped_dek: &WrappedDek,
    ) -> Result<WrappedDek, Error> {
        let dek = Self::unwrap_dek(old_kek, wrapped_dek)?;
        Self::wrap_dek(new_kek, &dek)
    }
}

use chacha20poly1305::{
    aead::{Aead, KeyInit},
    ChaCha20Poly1305, Key, Nonce,
};
use rand::RngCore;

use crate::types::{SecretError, SecretMaterial};

const KEY_SIZE: usize = 32;
const NONCE_SIZE: usize = 12;

/// AEAD Encryption/Decryption utility
#[derive(Debug)]
pub struct CryptoEngine {
    key: Key,
}

impl CryptoEngine {
    pub fn new(key_bytes: &[u8]) -> Result<Self, SecretError> {
        if key_bytes.len() != KEY_SIZE {
            return Err(SecretError::CryptoError(format!(
                "Invalid key size, expected {} bytes",
                KEY_SIZE
            )));
        }
        let key = Key::clone_from_slice(key_bytes);
        Ok(Self { key })
    }

    pub fn generate_key() -> Vec<u8> {
        let mut key = vec![0u8; KEY_SIZE];
        rand::thread_rng().fill_bytes(&mut key);
        key
    }

    pub fn encrypt(&self, plaintext: &SecretMaterial) -> Result<Vec<u8>, SecretError> {
        let cipher = ChaCha20Poly1305::new(&self.key);

        let mut nonce_bytes = [0u8; NONCE_SIZE];
        rand::thread_rng().fill_bytes(&mut nonce_bytes);
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| SecretError::CryptoError(format!("Encryption failed: {:?}", e)))?;

        // Format: [nonce][ciphertext]
        let mut result = Vec::with_capacity(NONCE_SIZE + ciphertext.len());
        result.extend_from_slice(&nonce_bytes);
        result.extend_from_slice(&ciphertext);

        Ok(result)
    }

    pub fn decrypt(&self, encrypted_data: &[u8]) -> Result<SecretMaterial, SecretError> {
        if encrypted_data.len() < NONCE_SIZE {
            return Err(SecretError::CryptoError("Ciphertext too short".into()));
        }

        let cipher = ChaCha20Poly1305::new(&self.key);
        let nonce = Nonce::from_slice(&encrypted_data[..NONCE_SIZE]);
        let ciphertext = &encrypted_data[NONCE_SIZE..];

        let plaintext = cipher
            .decrypt(nonce, ciphertext)
            .map_err(|_| SecretError::CryptoError("Decryption failed".into()))?;

        Ok(SecretMaterial::new(plaintext))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encryption_decryption() {
        let key = CryptoEngine::generate_key();
        let engine = CryptoEngine::new(&key).unwrap();

        let secret = SecretMaterial::new(b"super_secret_data".to_vec());
        let encrypted = engine.encrypt(&secret).unwrap();

        assert_ne!(encrypted, b"super_secret_data");
        assert!(encrypted.len() > b"super_secret_data".len());

        let decrypted = engine.decrypt(&encrypted).unwrap();
        assert_eq!(decrypted.as_bytes(), b"super_secret_data");
    }

    #[test]
    fn test_aead_misuse_wrong_key() {
        let key1 = CryptoEngine::generate_key();
        let key2 = CryptoEngine::generate_key();

        let engine1 = CryptoEngine::new(&key1).unwrap();
        let engine2 = CryptoEngine::new(&key2).unwrap();

        let secret = SecretMaterial::new(b"data".to_vec());
        let encrypted = engine1.encrypt(&secret).unwrap();

        // Attempt decrypt with different key should fail
        let result = engine2.decrypt(&encrypted);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecretError::CryptoError(_)));
    }

    #[test]
    fn test_aead_misuse_tampered_ciphertext() {
        let key = CryptoEngine::generate_key();
        let engine = CryptoEngine::new(&key).unwrap();

        let secret = SecretMaterial::new(b"data".to_vec());
        let mut encrypted = engine.encrypt(&secret).unwrap();

        // Tamper with the ciphertext
        let len = encrypted.len();
        encrypted[len - 1] ^= 1;

        let result = engine.decrypt(&encrypted);
        assert!(result.is_err());
    }

    #[test]
    fn test_aead_misuse_invalid_key_size() {
        let invalid_key = vec![0u8; 16]; // ChaCha20Poly1305 requires 32 bytes
        let result = CryptoEngine::new(&invalid_key);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), SecretError::CryptoError(_)));
    }
}

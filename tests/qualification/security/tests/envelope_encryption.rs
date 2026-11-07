#![forbid(unsafe_code)]

#[cfg(not(feature = "crypto"))]
#[test]
fn test_envelope_encryption_and_decryption() {
    panic!("RESULT=BLOCKED: Missing crypto dependencies");
}

#[cfg(not(feature = "crypto"))]
#[test]
fn test_atomic_key_rotation() {
    panic!("RESULT=BLOCKED: Missing crypto dependencies");
}

#[cfg(not(feature = "crypto"))]
#[test]
fn test_corrupted_ciphertext_detection() {
    panic!("RESULT=BLOCKED: Missing crypto dependencies");
}

#[cfg(feature = "crypto")]
mod crypto_tests {
    use aes_gcm::{
        aead::{Aead, AeadCore, KeyInit, OsRng},
        Aes256Gcm, Key
    };

    struct Envelope {
        encrypted_dek: Vec<u8>,
        dek_nonce: Vec<u8>,
        encrypted_payload: Vec<u8>,
        payload_nonce: Vec<u8>,
    }

    fn generate_key() -> Key<Aes256Gcm> {
        Aes256Gcm::generate_key(OsRng)
    }

    fn encrypt_with_key(key: &Key<Aes256Gcm>, data: &[u8]) -> (Vec<u8>, Vec<u8>) {
        let cipher = Aes256Gcm::new(key);
        let nonce = Aes256Gcm::generate_nonce(&mut OsRng); // 96-bits; unique per message
        let ciphertext = cipher.encrypt(&nonce, data).expect("encryption failure!");
        (ciphertext, nonce.to_vec())
    }

    fn decrypt_with_key(key: &Key<Aes256Gcm>, nonce_bytes: &[u8], ciphertext: &[u8]) -> Result<Vec<u8>, aes_gcm::Error> {
        let cipher = Aes256Gcm::new(key);
        let nonce = aes_gcm::Nonce::from_slice(nonce_bytes);
        cipher.decrypt(nonce, ciphertext)
    }

    #[test]
    fn test_envelope_encryption_and_decryption() {
        let master_key = generate_key(); // KEK
        let dek = generate_key();        // DEK
        let payload = b"sensitive user data";

        // Encrypt Payload with DEK
        let (encrypted_payload, payload_nonce) = encrypt_with_key(&dek, payload);

        // Encrypt DEK with KEK
        let (encrypted_dek, dek_nonce) = encrypt_with_key(&master_key, dek.as_slice());

        let envelope = Envelope {
            encrypted_dek,
            dek_nonce,
            encrypted_payload,
            payload_nonce,
        };

        // Decrypt DEK with KEK
        let decrypted_dek_bytes = decrypt_with_key(&master_key, &envelope.dek_nonce, &envelope.encrypted_dek).unwrap();
        let decrypted_dek = Key::<Aes256Gcm>::from_slice(&decrypted_dek_bytes);

        // Decrypt Payload with DEK
        let decrypted_payload = decrypt_with_key(decrypted_dek, &envelope.payload_nonce, &envelope.encrypted_payload).unwrap();

        assert_eq!(payload.as_slice(), decrypted_payload.as_slice());
    }

    #[test]
    fn test_atomic_key_rotation() {
        let old_kek = generate_key();
        let dek = generate_key();
        let payload = b"database password";

        // Encrypt Payload with DEK
        let (encrypted_payload, payload_nonce) = encrypt_with_key(&dek, payload);

        // Encrypt DEK with Old KEK
        let (encrypted_dek_old, dek_nonce_old) = encrypt_with_key(&old_kek, dek.as_slice());

        // Now, perform key rotation
        let new_kek = generate_key();

        // 1. Decrypt DEK using Old KEK
        let decrypted_dek_bytes = decrypt_with_key(&old_kek, &dek_nonce_old, &encrypted_dek_old).unwrap();

        // 2. Re-encrypt DEK using New KEK
        let (encrypted_dek_new, dek_nonce_new) = encrypt_with_key(&new_kek, &decrypted_dek_bytes);

        // 3. Verify that payload ciphertext remains untouched but we can still decrypt using new KEK envelope
        let decrypted_dek_bytes_new = decrypt_with_key(&new_kek, &dek_nonce_new, &encrypted_dek_new).unwrap();
        let decrypted_dek = Key::<Aes256Gcm>::from_slice(&decrypted_dek_bytes_new);

        let decrypted_payload = decrypt_with_key(decrypted_dek, &payload_nonce, &encrypted_payload).unwrap();
        assert_eq!(payload.as_slice(), decrypted_payload.as_slice());
    }

    #[test]
    fn test_corrupted_ciphertext_detection() {
        let dek = generate_key();
        let payload = b"financial records";

        let (mut encrypted_payload, payload_nonce) = encrypt_with_key(&dek, payload);

        // Corrupt the ciphertext (e.g. flip a bit in the last byte which is part of the auth tag)
        let last_idx = encrypted_payload.len() - 1;
        encrypted_payload[last_idx] ^= 1;

        // Attempt decryption
        let result = decrypt_with_key(&dek, &payload_nonce, &encrypted_payload);
        assert!(result.is_err(), "Decryption should fail due to corrupted ciphertext or tag");
    }
}

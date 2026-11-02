use argon2::password_hash::{
    rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString,
};
use argon2::Argon2;

use crate::error::IdentityError;

pub struct PasswordManager;

impl PasswordManager {
    /// Hashes a plain password using Argon2id with a cryptographically secure random salt.
    /// Secret parameters are never logged or stored in plaintext.
    pub fn hash_password(password: &str) -> Result<String, IdentityError> {
        if password.is_empty() {
            return Err(IdentityError::InvalidInput {
                message: "Password cannot be empty".to_string(),
            });
        }
        if password.len() < 8 {
            return Err(IdentityError::InvalidInput {
                message: "Password must be at least 8 characters long".to_string(),
            });
        }

        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();

        let password_hash = argon2
            .hash_password(password.as_bytes(), &salt)
            .map_err(|e| IdentityError::OperationFailed {
                message: format!("Password hashing failed: {}", e),
            })?
            .to_string();

        Ok(password_hash)
    }

    /// Verifies a plain password against an Argon2 PHC string.
    pub fn verify_password(password: &str, hash_str: &str) -> Result<(), IdentityError> {
        if password.is_empty() {
            return Err(IdentityError::InvalidCredentials);
        }

        let parsed_hash =
            PasswordHash::new(hash_str).map_err(|_| IdentityError::InvalidCredentials)?;

        Argon2::default()
            .verify_password(password.as_bytes(), &parsed_hash)
            .map_err(|_| IdentityError::InvalidCredentials)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hash_and_verify_success() {
        let password = "SuperSecretSecurePassword123!";
        let hash = PasswordManager::hash_password(password).unwrap();

        assert!(!hash.is_empty());
        assert!(hash.starts_with("$argon2id$"));
        assert!(PasswordManager::verify_password(password, &hash).is_ok());
    }

    #[test]
    fn test_password_verify_wrong_password() {
        let password = "SuperSecretSecurePassword123!";
        let hash = PasswordManager::hash_password(password).unwrap();

        let wrong = "WrongPassword456!";
        assert_eq!(
            PasswordManager::verify_password(wrong, &hash),
            Err(IdentityError::InvalidCredentials)
        );
    }

    #[test]
    fn test_password_length_validation() {
        assert!(matches!(
            PasswordManager::hash_password(""),
            Err(IdentityError::InvalidInput { .. })
        ));
        assert!(matches!(
            PasswordManager::hash_password("short"),
            Err(IdentityError::InvalidInput { .. })
        ));
    }

    #[test]
    fn test_password_malformed_hash() {
        assert_eq!(
            PasswordManager::verify_password("Password12345", "not-a-valid-argon2-hash"),
            Err(IdentityError::InvalidCredentials)
        );
    }
}

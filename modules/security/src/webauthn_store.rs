#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::time::{Duration, SystemTime};

#[derive(Debug, thiserror::Error, PartialEq)]
pub enum WebAuthnError {
    #[error("Challenge not found or expired")]
    InvalidChallenge,
    #[error("Credential not found")]
    CredentialNotFound,
    #[error("Replay detected: signature counter is not strictly increasing")]
    ReplayDetected,
    #[error("Credential already registered")]
    CredentialAlreadyRegistered,
    #[error("Internal RNG error")]
    RngError,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Credential {
    pub id: Vec<u8>,
    pub public_key: Vec<u8>,
    pub signature_counter: u32,
}

#[derive(Clone, Debug)]
pub struct Challenge {
    pub data: Vec<u8>,
    pub expires_at: SystemTime,
}

pub struct WebAuthnStore {
    credentials: HashMap<Vec<u8>, Credential>,
    challenges: HashMap<Vec<u8>, Challenge>,
}

impl Default for WebAuthnStore {
    fn default() -> Self {
        Self::new()
    }
}

impl WebAuthnStore {
    pub fn new() -> Self {
        Self {
            credentials: HashMap::new(),
            challenges: HashMap::new(),
        }
    }

    pub fn generate_challenge(&mut self) -> Result<Vec<u8>, WebAuthnError> {
        let mut buf = [0u8; 32];
        let mut f = File::open("/dev/urandom").map_err(|_| WebAuthnError::RngError)?;
        f.read_exact(&mut buf).map_err(|_| WebAuthnError::RngError)?;

        let challenge = buf.to_vec();
        let expires_at = SystemTime::now() + Duration::from_secs(60);

        self.challenges.insert(challenge.clone(), Challenge {
            data: challenge.clone(),
            expires_at,
        });

        Ok(challenge)
    }

    pub fn verify_challenge(&mut self, challenge_data: &[u8]) -> Result<(), WebAuthnError> {
        self.verify_challenge_at(challenge_data, SystemTime::now())
    }

    pub fn verify_challenge_at(&mut self, challenge_data: &[u8], now: SystemTime) -> Result<(), WebAuthnError> {
        if let Some(challenge) = self.challenges.remove(challenge_data) {
            if challenge.expires_at >= now {
                Ok(())
            } else {
                Err(WebAuthnError::InvalidChallenge)
            }
        } else {
            Err(WebAuthnError::InvalidChallenge)
        }
    }

    pub fn register_credential(&mut self, id: Vec<u8>, public_key: Vec<u8>, initial_counter: u32) -> Result<(), WebAuthnError> {
        if self.credentials.contains_key(&id) {
            return Err(WebAuthnError::CredentialAlreadyRegistered);
        }

        self.credentials.insert(id.clone(), Credential {
            id,
            public_key,
            signature_counter: initial_counter,
        });

        Ok(())
    }

    pub fn update_signature_counter(&mut self, id: &[u8], new_counter: u32) -> Result<(), WebAuthnError> {
        if let Some(cred) = self.credentials.get_mut(id) {
            // Strictly increasing counter, or zero if authenticator doesn't support counters (always 0)
            if new_counter > cred.signature_counter || (cred.signature_counter == 0 && new_counter == 0) {
                cred.signature_counter = new_counter;
                Ok(())
            } else {
                Err(WebAuthnError::ReplayDetected)
            }
        } else {
            Err(WebAuthnError::CredentialNotFound)
        }
    }

    pub fn get_credential(&self, id: &[u8]) -> Option<&Credential> {
        self.credentials.get(id)
    }

    pub fn cleanup_expired_challenges(&mut self) {
        let now = SystemTime::now();
        self.challenges.retain(|_, v| v.expires_at >= now);
    }
}

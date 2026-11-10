#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use rand::RngCore;
use std::collections::VecDeque;
use std::fs;
use std::path::Path;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum StekError {
    #[error("Failed to generate key")]
    KeyGenerationError,
    #[error("Key is expired")]
    KeyExpired,
    #[error("Invalid key size: expected {0}")]
    InvalidKeySize(usize),
    #[error("IO Error: {0}")]
    IoError(#[from] std::io::Error),
}

pub const TICKET_KEY_SIZE: usize = 48; // 16 name + 16 iv/aes + 16 hmac

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyState {
    Primary,
    Secondary,
    Expired,
}

#[derive(Debug, Clone)]
pub struct TicketKey {
    pub data: [u8; TICKET_KEY_SIZE],
    pub state: KeyState,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}

impl TicketKey {
    pub fn generate(ttl_seconds: i64) -> Result<Self, StekError> {
        let mut data = [0u8; TICKET_KEY_SIZE];
        rand::thread_rng().fill_bytes(&mut data);

        let now = Utc::now();
        let expires_at = now + chrono::Duration::seconds(ttl_seconds);

        Ok(TicketKey {
            data,
            state: KeyState::Primary, // New keys are primary
            created_at: now,
            expires_at,
        })
    }

    pub fn get_name(&self) -> &[u8] {
        &self.data[0..16]
    }
}

pub struct StekManager {
    keys: VecDeque<TicketKey>,
    pub rotation_interval_seconds: i64,
    pub expiration_ttl_seconds: i64,
    pub last_rotation_at: Option<DateTime<Utc>>,
}

impl StekManager {
    pub fn new(rotation_interval_seconds: i64, expiration_ttl_seconds: i64) -> Self {
        Self {
            keys: VecDeque::new(),
            rotation_interval_seconds,
            expiration_ttl_seconds,
            last_rotation_at: None,
        }
    }

    pub fn maybe_rotate(&mut self) -> Result<bool, StekError> {
        let now = Utc::now();
        if let Some(last_rotation) = self.last_rotation_at {
             if (now - last_rotation).num_seconds() >= self.rotation_interval_seconds {
                 self.rotate_keys()?;
                 return Ok(true);
             }
        } else {
             self.rotate_keys()?;
             return Ok(true);
        }

        Ok(false)
    }

    pub fn rotate_keys(&mut self) -> Result<(), StekError> {
        let now = Utc::now();

        // Mark current keys
        for key in &mut self.keys {
            if key.state == KeyState::Primary {
                key.state = KeyState::Secondary;
            }
            if key.expires_at <= now {
                key.state = KeyState::Expired;
            }
        }

        // Generate new primary key
        let new_key = TicketKey::generate(self.expiration_ttl_seconds)?;
        self.keys.push_front(new_key);
        self.last_rotation_at = Some(now);

        Ok(())
    }

    pub fn prune_expired(&mut self) {
        let now = Utc::now();
        self.keys.retain(|k| k.expires_at > now);
    }

    pub fn get_primary(&self) -> Option<&TicketKey> {
        self.keys.iter().find(|k| k.state == KeyState::Primary && k.expires_at > Utc::now())
    }

    pub fn get_valid_keys(&self) -> Vec<&TicketKey> {
        let now = Utc::now();
        self.keys.iter().filter(|k| k.state != KeyState::Expired && k.expires_at > now).collect()
    }

    pub fn lookup_key(&self, name: &[u8]) -> Option<&TicketKey> {
        self.get_valid_keys().into_iter().find(|k| k.get_name() == name)
    }

    pub fn save_keys_to_disk(&self, dir: &Path) -> Result<(), StekError> {
        if !dir.exists() {
            fs::create_dir_all(dir)?;
        }
        for key in self.get_valid_keys() {
            let file_path = dir.join(format!("{:x}.key", key.created_at.timestamp_micros()));
            fs::write(file_path, key.data)?;
        }
        Ok(())
    }

    pub fn generate_nginx_config(&self, ticket_file_path: &str) -> String {
        let mut config = String::new();
        // Nginx reads keys in order. We should give them in the order we want to use them.
        for key in self.get_valid_keys() {
             config.push_str(&format!("ssl_session_ticket_key {}/{:x}.key;\n", ticket_file_path, key.created_at.timestamp_micros()));
        }
        config
    }

    pub fn generate_apache_config(&self, ticket_file_path: &str) -> String {
        let mut config = String::new();
        for key in self.get_valid_keys() {
            config.push_str(&format!("SSLSessionTicketKeyFile {}/{:x}.key\n", ticket_file_path, key.created_at.timestamp_micros()));
        }
        config
    }
}

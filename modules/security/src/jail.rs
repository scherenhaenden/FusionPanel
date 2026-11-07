#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::{Duration, SystemTime};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum JailError {
    #[error("IP is currently banned")]
    IpBanned,
    #[error("IP is not currently banned")]
    IpNotBanned,
}

#[derive(Debug, Clone)]
pub struct JailConfig {
    pub maxretry: u32,
    pub findtime: Duration,
    pub bantime: Duration,
}

impl Default for JailConfig {
    fn default() -> Self {
        Self {
            maxretry: 3,
            findtime: Duration::from_secs(600), // 10 minutes
            bantime: Duration::from_secs(3600), // 1 hour
        }
    }
}

pub struct JailManager {
    config: JailConfig,
    failed_attempts: HashMap<String, Vec<SystemTime>>,
    banned_ips: HashMap<String, SystemTime>,
}

impl JailManager {
    pub fn new(config: JailConfig) -> Self {
        Self {
            config,
            failed_attempts: HashMap::new(),
            banned_ips: HashMap::new(),
        }
    }

    /// Clean up expired bans and old failed attempts
    pub fn cleanup(&mut self, now: SystemTime) {
        // Remove expired bans
        self.banned_ips.retain(|_, &mut expire_time| expire_time > now);

        // Remove old failed attempts outside of findtime
        let findtime = self.config.findtime;
        self.failed_attempts.retain(|_, attempts| {
            attempts.retain(|&attempt_time| {
                if let Ok(elapsed) = now.duration_since(attempt_time) {
                    elapsed <= findtime
                } else {
                    false // if attempt time is in the future, remove it just in case
                }
            });
            !attempts.is_empty()
        });
    }

    /// Record a failed authentication attempt
    pub fn record_failure(&mut self, ip: &str, now: SystemTime) -> Result<bool, JailError> {
        self.cleanup(now);

        if self.is_banned(ip, now) {
            return Ok(true); // Already banned
        }

        let attempts = self.failed_attempts.entry(ip.to_string()).or_insert_with(Vec::new);
        attempts.push(now);

        if attempts.len() as u32 >= self.config.maxretry {
            // Trigger ban
            let expire_time = now + self.config.bantime;
            self.banned_ips.insert(ip.to_string(), expire_time);
            self.failed_attempts.remove(ip);
            return Ok(true); // Was banned
        }

        Ok(false) // Not banned
    }

    /// Check if an IP is currently banned
    pub fn is_banned(&mut self, ip: &str, now: SystemTime) -> bool {
        self.cleanup(now);
        self.banned_ips.contains_key(ip)
    }

    /// Manually ban an IP address
    pub fn ban(&mut self, ip: &str, now: SystemTime) {
        let expire_time = now + self.config.bantime;
        self.banned_ips.insert(ip.to_string(), expire_time);
        self.failed_attempts.remove(ip);
    }

    /// Manually unban an IP address
    pub fn unban(&mut self, ip: &str) -> Result<(), JailError> {
        if self.banned_ips.remove(ip).is_some() {
            Ok(())
        } else {
            Err(JailError::IpNotBanned)
        }
    }

    /// Get active ban list
    pub fn active_bans(&mut self, now: SystemTime) -> Vec<String> {
        self.cleanup(now);
        self.banned_ips.keys().cloned().collect()
    }
}

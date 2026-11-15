#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::{Duration, Instant};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum KnockProtocol {
    Tcp,
    Udp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KnockConfig {
    /// Expected sequence of knocks (port, protocol)
    pub sequence: Vec<(u16, KnockProtocol)>,
    /// Maximum time allowed between consecutive knocks
    pub knock_timeout: Duration,
    /// Duration for which port 22 access is granted upon successful knock
    pub grant_duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnockState {
    /// Index in the sequence the user is currently at
    pub current_index: usize,
    /// Timestamp of the last successful knock
    pub last_knock: Instant,
}

#[derive(Debug)]
pub struct PortKnocker {
    pub config: KnockConfig,
    /// Maps IP address (String representation for simplicity, e.g., "192.168.1.1") to their current knock state
    active_knocks: HashMap<String, KnockState>,
    /// Maps IP address to the time when their granted access expires
    granted_access: HashMap<String, Instant>,
}

pub enum KnockResult {
    /// The knock sequence was completed and access is granted
    AccessGranted,
    /// The knock advanced the sequence, but it is not yet complete
    Advanced,
    /// The knock was invalid and the sequence was reset
    Reset,
    /// Access has already been granted and is currently active
    AlreadyGranted,
}

impl PortKnocker {
    pub fn new(config: KnockConfig) -> Self {
        Self {
            config,
            active_knocks: HashMap::new(),
            granted_access: HashMap::new(),
        }
    }

    /// Process an incoming port knock attempt.
    pub fn process_knock(&mut self, ip: &str, port: u16, protocol: KnockProtocol, now: Instant) -> KnockResult {
        // Clean up expired grants first
        self.cleanup(now);

        // Check if access is already granted
        if let Some(&expiry) = self.granted_access.get(ip) {
            if now < expiry {
                return KnockResult::AlreadyGranted;
            }
        }

        if self.config.sequence.is_empty() {
            return KnockResult::Reset;
        }

        let mut current_state = self.active_knocks.remove(ip);

        // If a state exists, check for timeout
        if let Some(state) = &current_state {
            if now.saturating_duration_since(state.last_knock) > self.config.knock_timeout {
                current_state = None; // Reset due to timeout
            }
        }

        let expected_index = current_state.map(|s| s.current_index).unwrap_or(0);
        let expected_knock = &self.config.sequence[expected_index];

        if port == expected_knock.0 && protocol == expected_knock.1 {
            let next_index = expected_index + 1;

            if next_index == self.config.sequence.len() {
                // Sequence completed
                self.granted_access.insert(ip.to_string(), now + self.config.grant_duration);
                KnockResult::AccessGranted
            } else {
                // Sequence advanced
                self.active_knocks.insert(
                    ip.to_string(),
                    KnockState {
                        current_index: next_index,
                        last_knock: now,
                    },
                );
                KnockResult::Advanced
            }
        } else {
            // Incorrect knock, if we expected the first knock, check if it matches the first knock
            let first_knock = &self.config.sequence[0];
            if port == first_knock.0 && protocol == first_knock.1 {
                 self.active_knocks.insert(
                    ip.to_string(),
                    KnockState {
                        current_index: 1,
                        last_knock: now,
                    },
                );
                KnockResult::Advanced
            } else {
                // Reset state
                KnockResult::Reset
            }
        }
    }

    /// Remove expired state
    pub fn cleanup(&mut self, now: Instant) {
        let timeout = self.config.knock_timeout;
        self.active_knocks.retain(|_, state| now.saturating_duration_since(state.last_knock) <= timeout);
        self.granted_access.retain(|_, expiry| now < *expiry);
    }

    pub fn is_granted(&self, ip: &str, now: Instant) -> bool {
        self.granted_access.get(ip).map_or(false, |&expiry| now < expiry)
    }

    /// Generates iptables commands to allow port 22 for all currently granted IPs.
    pub fn generate_firewall_rules(&self, now: Instant) -> Vec<String> {
        let mut rules = Vec::new();
        for (ip, &expiry) in &self.granted_access {
            if now < expiry {
                rules.push(format!("iptables -I INPUT -s {} -p tcp --dport 22 -j ACCEPT", ip));
            }
        }
        rules.sort(); // Ensure deterministic output for testing
        rules
    }
}

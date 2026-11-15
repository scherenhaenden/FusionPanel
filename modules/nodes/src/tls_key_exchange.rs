#![forbid(unsafe_code)]

use std::collections::HashMap;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum RekeyError {
    #[error("Session not found for node {0}")]
    SessionNotFound(String),
    #[error("Epoch {0} is expired or invalid")]
    InvalidEpoch(u64),
}

#[derive(Debug, Clone)]
pub struct RekeyPolicy {
    pub max_bytes: u64,
    pub max_packets: u64,
    pub ttl_secs: u64,
    pub transition_window_secs: u64,
}

impl Default for RekeyPolicy {
    fn default() -> Self {
        Self {
            max_bytes: 1024 * 1024 * 1024, // 1 GB
            max_packets: 1_000_000,
            ttl_secs: 3600,             // 1 hour
            transition_window_secs: 60, // 1 minute
        }
    }
}

pub trait RekeyEventCallback {
    fn on_rekey_completed(&self, node_id: &str, new_epoch: u64);
    fn on_epoch_timeout(&self, node_id: &str, expired_epoch: u64);
}

pub struct NodeSession {
    pub active_epoch: u64,
    pub previous_epoch: Option<u64>,

    pub bytes_since_rekey: u64,
    pub packets_since_rekey: u64,
    pub epoch_start_time: u64,

    pub previous_epoch_expiry: Option<u64>,
}

impl NodeSession {
    pub fn new(epoch_start_time: u64, initial_epoch: u64) -> Self {
        Self {
            active_epoch: initial_epoch,
            previous_epoch: None,
            bytes_since_rekey: 0,
            packets_since_rekey: 0,
            epoch_start_time,
            previous_epoch_expiry: None,
        }
    }
}

pub struct MeshRekeyScheduler<C: RekeyEventCallback> {
    pub policy: RekeyPolicy,
    pub sessions: HashMap<String, NodeSession>,
    pub callback: C,
}

impl<C: RekeyEventCallback> MeshRekeyScheduler<C> {
    pub fn new(policy: RekeyPolicy, callback: C) -> Self {
        Self {
            policy,
            sessions: HashMap::new(),
            callback,
        }
    }

    pub fn register_node(&mut self, node_id: String, initial_epoch: u64, current_time: u64) {
        self.sessions
            .insert(node_id, NodeSession::new(current_time, initial_epoch));
    }

    pub fn unregister_node(&mut self, node_id: &str) {
        self.sessions.remove(node_id);
    }

    /// Records transfer and checks if rekey is needed. If true, triggers rekey.
    pub fn record_transfer(
        &mut self,
        node_id: &str,
        bytes: u64,
        current_time: u64,
    ) -> Result<bool, RekeyError> {
        let session = self
            .sessions
            .get_mut(node_id)
            .ok_or_else(|| RekeyError::SessionNotFound(node_id.to_string()))?;
        session.bytes_since_rekey += bytes;
        session.packets_since_rekey += 1;

        let needs_rekey = session.bytes_since_rekey >= self.policy.max_bytes
            || session.packets_since_rekey >= self.policy.max_packets
            || (current_time.saturating_sub(session.epoch_start_time)) >= self.policy.ttl_secs;

        if needs_rekey {
            // Drop mutable borrow to call trigger_rekey
            self.trigger_rekey(node_id, current_time)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn check_ttl(&mut self, node_id: &str, current_time: u64) -> Result<bool, RekeyError> {
        let session = self
            .sessions
            .get(node_id)
            .ok_or_else(|| RekeyError::SessionNotFound(node_id.to_string()))?;
        let needs_rekey =
            (current_time.saturating_sub(session.epoch_start_time)) >= self.policy.ttl_secs;

        if needs_rekey {
            self.trigger_rekey(node_id, current_time)?;
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn trigger_rekey(&mut self, node_id: &str, current_time: u64) -> Result<(), RekeyError> {
        let session = self
            .sessions
            .get_mut(node_id)
            .ok_or_else(|| RekeyError::SessionNotFound(node_id.to_string()))?;

        let old_epoch = session.active_epoch;
        let new_epoch = old_epoch + 1;

        session.previous_epoch = Some(old_epoch);
        session.previous_epoch_expiry = Some(current_time + self.policy.transition_window_secs);
        session.active_epoch = new_epoch;
        session.bytes_since_rekey = 0;
        session.packets_since_rekey = 0;
        session.epoch_start_time = current_time;

        self.callback.on_rekey_completed(node_id, new_epoch);
        Ok(())
    }

    pub fn evict_expired_epochs(&mut self, current_time: u64) {
        for (node_id, session) in self.sessions.iter_mut() {
            if let Some(expiry) = session.previous_epoch_expiry {
                if current_time >= expiry {
                    if let Some(expired_epoch) = session.previous_epoch.take() {
                        self.callback.on_epoch_timeout(node_id, expired_epoch);
                    }
                    session.previous_epoch_expiry = None;
                }
            }
        }
    }

    pub fn resolve_epoch(
        &self,
        node_id: &str,
        epoch: u64,
        current_time: u64,
    ) -> Result<bool, RekeyError> {
        let session = self
            .sessions
            .get(node_id)
            .ok_or_else(|| RekeyError::SessionNotFound(node_id.to_string()))?;
        if session.active_epoch == epoch {
            return Ok(true);
        }

        if let Some(prev) = session.previous_epoch {
            if prev == epoch {
                if let Some(expiry) = session.previous_epoch_expiry {
                    if current_time < expiry {
                        return Ok(true);
                    }
                }
            }
        }
        Err(RekeyError::InvalidEpoch(epoch))
    }
}

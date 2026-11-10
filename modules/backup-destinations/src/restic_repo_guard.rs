#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;
use thiserror::Error;

#[derive(Error, Debug, PartialEq)]
pub enum LockError {
    #[error("Failed to parse lock metadata: {0}")]
    ParseError(String),
    #[error("Conflict: exclusive lock already held by lock {0}")]
    ExclusiveLockConflict(String),
    #[error("Conflict: shared lock(s) currently held")]
    SharedLockConflict,
    #[error("Lock not found: {0}")]
    NotFound(String),
    #[error("Unauthorized unlock attempt for lock: {0}")]
    UnauthorizedUnlock(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LockType {
    Exclusive,
    Shared,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResticLockMetadata {
    pub time: DateTime<Utc>,
    #[serde(default)]
    pub exclusive: bool,
    pub hostname: String,
    pub username: String,
    pub pid: u32,
    pub uid: u32,
    pub gid: u32,
}

#[derive(Debug, Clone)]
pub struct ResticLock {
    pub id: String,
    pub lock_type: LockType,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub pid: u32,
    pub hostname: String,
}

impl ResticLock {
    pub fn is_stale(&self, now: DateTime<Utc>, ttl: Duration) -> bool {
        if let Ok(duration) = now.signed_duration_since(self.updated_at).to_std() {
            duration > ttl
        } else {
            false
        }
    }
}

pub struct ResticLockGuard {
    locks: HashMap<String, ResticLock>,
    stale_ttl: Duration,
}

impl ResticLockGuard {
    pub fn new(stale_ttl: Duration) -> Self {
        Self {
            locks: HashMap::new(),
            stale_ttl,
        }
    }

    pub fn parse_lock(id: &str, json_data: &str, updated_at: DateTime<Utc>) -> Result<ResticLock, LockError> {
        let metadata: ResticLockMetadata = serde_json::from_str(json_data)
            .map_err(|e| LockError::ParseError(e.to_string()))?;

        let lock_type = if metadata.exclusive {
            LockType::Exclusive
        } else {
            LockType::Shared
        };

        Ok(ResticLock {
            id: id.to_string(),
            lock_type,
            created_at: metadata.time,
            updated_at,
            pid: metadata.pid,
            hostname: metadata.hostname,
        })
    }

    pub fn load_lock(&mut self, lock: ResticLock, now: DateTime<Utc>) {
        self.cleanup_stale_locks(now);

        if !lock.is_stale(now, self.stale_ttl) {
             self.locks.insert(lock.id.clone(), lock);
        }
    }

    pub fn acquire_lock(&mut self, lock: ResticLock, now: DateTime<Utc>) -> Result<(), LockError> {
        self.cleanup_stale_locks(now);

        if lock.lock_type == LockType::Exclusive {
            if let Some(holder) = self.locks.values().find(|l| l.lock_type == LockType::Exclusive) {
                return Err(LockError::ExclusiveLockConflict(holder.id.clone()));
            }
            if self.locks.values().any(|l| l.lock_type == LockType::Shared) {
                return Err(LockError::SharedLockConflict);
            }
        } else {
            if let Some(holder) = self.locks.values().find(|l| l.lock_type == LockType::Exclusive) {
                return Err(LockError::ExclusiveLockConflict(holder.id.clone()));
            }
        }

        self.locks.insert(lock.id.clone(), lock);
        Ok(())
    }

    pub fn release_lock(&mut self, id: &str, hostname: &str, pid: u32) -> Result<(), LockError> {
        if let Some(lock) = self.locks.get(id) {
            if lock.hostname == hostname && lock.pid == pid {
                self.locks.remove(id);
                Ok(())
            } else {
                Err(LockError::UnauthorizedUnlock(id.to_string()))
            }
        } else {
            Err(LockError::NotFound(id.to_string()))
        }
    }

    pub fn refresh_lock(&mut self, id: &str, hostname: &str, pid: u32, now: DateTime<Utc>) -> Result<(), LockError> {
        if let Some(lock) = self.locks.get_mut(id) {
            if lock.hostname == hostname && lock.pid == pid {
                lock.updated_at = now;
                Ok(())
            } else {
                Err(LockError::UnauthorizedUnlock(id.to_string()))
            }
        } else {
            Err(LockError::NotFound(id.to_string()))
        }
    }

    pub fn cleanup_stale_locks(&mut self, now: DateTime<Utc>) -> usize {
        let mut stale_ids = Vec::new();
        for (id, lock) in &self.locks {
            if lock.is_stale(now, self.stale_ttl) {
                stale_ids.push(id.clone());
            }
        }
        for id in &stale_ids {
            self.locks.remove(id);
        }
        stale_ids.len()
    }

    pub fn get_locks(&self) -> Vec<&ResticLock> {
        self.locks.values().collect()
    }
}

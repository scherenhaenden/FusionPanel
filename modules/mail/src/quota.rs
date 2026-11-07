#![forbid(unsafe_code)]

use crate::error::MailError;
use serde::{Deserialize, Serialize};

/// MailboxQuota specifying max bytes and max message count for a virtual mailbox or domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MailboxQuota {
    pub max_bytes: u64,
    pub max_messages: u64,
}

/// Tracks current usage of a mailbox or domain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct QuotaUsage {
    pub used_bytes: u64,
    pub used_messages: u64,
}

/// Computed status representing available capacity and usage percentages.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaStatus {
    pub available_bytes: u64,
    pub available_messages: u64,
    pub bytes_usage_percent: f64,
    pub messages_usage_percent: f64,
    pub is_exceeded: bool,
}

/// Represents the triggered quota warning.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum QuotaWarning {
    None,
    Warning(f64),
    Critical(f64),
    Exceeded,
}

/// Configurable thresholds for warnings (e.g., 80% and 90%).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QuotaThresholds {
    pub warning: f64,
    pub critical: f64,
}

impl Default for QuotaThresholds {
    fn default() -> Self {
        Self {
            warning: 80.0,
            critical: 90.0,
        }
    }
}

/// Tracks and calculates quota usage, available capacity, and warnings.
#[derive(Debug, Clone)]
pub struct QuotaTracker {
    pub quota: MailboxQuota,
    pub thresholds: QuotaThresholds,
}

impl QuotaTracker {
    pub fn new(quota: MailboxQuota, thresholds: QuotaThresholds) -> Self {
        Self { quota, thresholds }
    }

    /// Computes the current status, including available capacity and percentages.
    pub fn compute_status(&self, usage: &QuotaUsage) -> QuotaStatus {
        let available_bytes = self.quota.max_bytes.saturating_sub(usage.used_bytes);
        let available_messages = self.quota.max_messages.saturating_sub(usage.used_messages);

        let bytes_usage_percent = if self.quota.max_bytes == 0 {
            0.0
        } else {
            (usage.used_bytes as f64 / self.quota.max_bytes as f64) * 100.0
        };

        let messages_usage_percent = if self.quota.max_messages == 0 {
            0.0
        } else {
            (usage.used_messages as f64 / self.quota.max_messages as f64) * 100.0
        };

        let is_exceeded = usage.used_bytes > self.quota.max_bytes
            || usage.used_messages > self.quota.max_messages;

        QuotaStatus {
            available_bytes,
            available_messages,
            bytes_usage_percent,
            messages_usage_percent,
            is_exceeded,
        }
    }

    /// Checks if any warning threshold has been reached based on current usage.
    pub fn check_warning(&self, usage: &QuotaUsage) -> QuotaWarning {
        let status = self.compute_status(usage);
        if status.is_exceeded {
            return QuotaWarning::Exceeded;
        }

        let max_percent = status
            .bytes_usage_percent
            .max(status.messages_usage_percent);

        if max_percent >= self.thresholds.critical {
            QuotaWarning::Critical(max_percent)
        } else if max_percent >= self.thresholds.warning {
            QuotaWarning::Warning(max_percent)
        } else {
            QuotaWarning::None
        }
    }

    /// Validates whether a new incoming message can be delivered.
    /// Rejects with an error if the new message exceeds the quota.
    pub fn check_delivery(&self, usage: &QuotaUsage, incoming_bytes: u64) -> Result<(), MailError> {
        let new_bytes = usage.used_bytes.saturating_add(incoming_bytes);
        let new_messages = usage.used_messages.saturating_add(1);

        if new_bytes > self.quota.max_bytes {
            return Err(MailError::Conflict(format!(
                "Quota exceeded: Max bytes {}, requested {}",
                self.quota.max_bytes, new_bytes
            )));
        }

        if new_messages > self.quota.max_messages {
            return Err(MailError::Conflict(format!(
                "Quota exceeded: Max messages {}, requested {}",
                self.quota.max_messages, new_messages
            )));
        }

        Ok(())
    }
}

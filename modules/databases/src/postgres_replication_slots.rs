#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicationSlot {
    pub slot_name: String,
    pub plugin: Option<String>,
    pub slot_type: String,
    pub active: bool,
    pub wal_status: String,
    pub safe_wal_size: Option<u64>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SlotError {
    #[error("Invalid slot name: must match ^[a-zA-Z0-9_]{{1,63}}$")]
    InvalidSlotName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SlotHealth {
    Healthy,
    Warning(String),
    Critical(String),
}

pub struct ReplicationSlotMonitor;

impl ReplicationSlotMonitor {
    pub fn new() -> Self {
        Self
    }

    pub fn evaluate_health(slot: &ReplicationSlot) -> SlotHealth {
        if slot.wal_status == "lost" {
            return SlotHealth::Critical(format!("Slot '{}' has lost WAL status", slot.slot_name));
        }

        if slot.wal_status == "extended" || slot.wal_status == "unreserved" {
            return SlotHealth::Warning(format!(
                "Slot '{}' has {} WAL status",
                slot.slot_name, slot.wal_status
            ));
        }

        if !slot.active {
            return SlotHealth::Warning(format!("Slot '{}' is inactive", slot.slot_name));
        }

        SlotHealth::Healthy
    }

    pub fn format_drop_slot(slot_name: &str) -> Result<String, SlotError> {
        if slot_name.is_empty() || slot_name.len() > 63 {
            return Err(SlotError::InvalidSlotName);
        }

        for c in slot_name.chars() {
            if !c.is_ascii_alphanumeric() && c != '_' {
                return Err(SlotError::InvalidSlotName);
            }
        }

        Ok(format!("SELECT pg_drop_replication_slot('{}');", slot_name))
    }
}

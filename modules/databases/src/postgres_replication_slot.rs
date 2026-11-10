#![forbid(unsafe_code)]

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MonitorError {
    #[error("Invalid LSN format: expected format 'X/Y'")]
    InvalidLsnFormat,
    #[error("Parse error for LSN part: {0}")]
    LsnParseError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LagSeverity {
    Normal,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecommendedAction {
    SafeDrop,
    ActiveReplicaReconnect,
    Pause,
    None,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicationSlot {
    pub slot_name: String,
    pub plugin: Option<String>,
    pub slot_type: String, // "physical" or "logical"
    pub active: bool,
    pub restart_lsn: Option<String>,
    pub confirmed_flush_lsn: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotEvaluation {
    pub slot_name: String,
    pub lag_bytes: Option<u64>,
    pub severity: LagSeverity,
    pub action: RecommendedAction,
}

pub struct ReplicationSlotMonitor {
    pub warning_threshold_bytes: u64,
    pub critical_threshold_bytes: u64,
}

impl ReplicationSlotMonitor {
    pub fn new(warning_threshold_bytes: u64, critical_threshold_bytes: u64) -> Self {
        Self {
            warning_threshold_bytes,
            critical_threshold_bytes,
        }
    }

    /// Parses an LSN string like "0/16B3748" to a u64 representing the byte offset.
    pub fn parse_lsn(lsn: &str) -> Result<u64, MonitorError> {
        let parts: Vec<&str> = lsn.split('/').collect();
        if parts.len() != 2 {
            return Err(MonitorError::InvalidLsnFormat);
        }

        let logical_id = u32::from_str_radix(parts[0], 16)
            .map_err(|e| MonitorError::LsnParseError(e.to_string()))?;
        let offset = u32::from_str_radix(parts[1], 16)
            .map_err(|e| MonitorError::LsnParseError(e.to_string()))?;

        Ok(((logical_id as u64) << 32) | (offset as u64))
    }

    pub fn calculate_lag(current_lsn: &str, slot_lsn: &str) -> Result<u64, MonitorError> {
        let current = Self::parse_lsn(current_lsn)?;
        let slot = Self::parse_lsn(slot_lsn)?;

        // Return 0 if slot LSN is somehow ahead of current LSN
        Ok(current.saturating_sub(slot))
    }

    pub fn evaluate_slot(&self, current_lsn: &str, slot: &ReplicationSlot) -> Result<SlotEvaluation, MonitorError> {
        let lsn_to_compare = if slot.slot_type == "logical" {
            slot.confirmed_flush_lsn.as_deref().or(slot.restart_lsn.as_deref())
        } else {
            slot.restart_lsn.as_deref()
        };

        let lag_bytes = match lsn_to_compare {
            Some(lsn) => Some(Self::calculate_lag(current_lsn, lsn)?),
            None => None,
        };

        let severity = match lag_bytes {
            Some(lag) if lag >= self.critical_threshold_bytes => LagSeverity::Critical,
            Some(lag) if lag >= self.warning_threshold_bytes => LagSeverity::Warning,
            _ => LagSeverity::Normal,
        };

        let action = match (slot.active, &severity) {
            (false, LagSeverity::Critical | LagSeverity::Warning) => {
                if slot.slot_type == "physical" {
                    RecommendedAction::ActiveReplicaReconnect
                } else {
                    RecommendedAction::SafeDrop
                }
            }
            (false, LagSeverity::Normal) => {
                RecommendedAction::SafeDrop
            }
            (true, LagSeverity::Critical) => RecommendedAction::Pause,
            (true, _) => RecommendedAction::None,
        };

        Ok(SlotEvaluation {
            slot_name: slot.slot_name.clone(),
            lag_bytes,
            severity,
            action,
        })
    }
}

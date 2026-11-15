#![forbid(unsafe_code)]

use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReplicationState {
    Startup,
    Catchup,
    Streaming,
    Backup,
    Stopping,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lsn(pub u64);

impl Lsn {
    pub fn from_str(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return None;
        }
        let log = u32::from_str_radix(parts[0], 16).ok()?;
        let offset = u32::from_str_radix(parts[1], 16).ok()?;
        Some(Lsn(((log as u64) << 32) | (offset as u64)))
    }

    pub fn diff(&self, other: &Lsn) -> u64 {
        self.0.saturating_sub(other.0)
    }
}

#[derive(Debug, Clone)]
pub struct PgStatReplication {
    pub application_name: String,
    pub client_addr: String,
    pub state: ReplicationState,
    pub sent_lsn: Lsn,
    pub write_lsn: Lsn,
    pub flush_lsn: Lsn,
    pub replay_lsn: Lsn,
    pub write_lag: Option<Duration>,
    pub flush_lag: Option<Duration>,
    pub replay_lag: Option<Duration>,
}

#[derive(Debug, Clone)]
pub struct ReplicationThresholds {
    pub warning_bytes: u64,
    pub critical_bytes: u64,
    pub warning_time: Duration,
    pub critical_time: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LagSeverity {
    Ok,
    Warning,
    Critical,
}

impl PgStatReplication {
    pub fn byte_lag(&self) -> u64 {
        self.sent_lsn.diff(&self.replay_lsn)
    }

    pub fn evaluate_lag(&self, thresholds: &ReplicationThresholds) -> LagSeverity {
        let lag_bytes = self.byte_lag();
        let lag_time = self.replay_lag.unwrap_or(Duration::ZERO);

        if lag_bytes >= thresholds.critical_bytes || lag_time >= thresholds.critical_time {
            LagSeverity::Critical
        } else if lag_bytes >= thresholds.warning_bytes || lag_time >= thresholds.warning_time {
            LagSeverity::Warning
        } else {
            LagSeverity::Ok
        }
    }
}

pub fn is_valid_identifier(ident: &str) -> bool {
    if ident.is_empty() || ident.len() > 63 {
        return false;
    }
    for c in ident.chars() {
        if !c.is_ascii_alphanumeric() && c != '_' {
            return false;
        }
    }
    true
}

pub fn generate_alter_role_connection_limit(
    role: &str,
    limit: i32,
) -> Result<String, &'static str> {
    if !is_valid_identifier(role) {
        return Err("Invalid role identifier");
    }
    Ok(format!("ALTER ROLE {} CONNECTION LIMIT {};", role, limit))
}

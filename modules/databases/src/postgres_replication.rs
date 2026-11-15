#![forbid(unsafe_code)]

use std::num::ParseIntError;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lsn(u64);

impl Lsn {
    pub fn new(val: u64) -> Self {
        Lsn(val)
    }

    pub fn value(&self) -> u64 {
        self.0
    }

    pub fn diff(&self, other: &Lsn) -> u64 {
        if self.0 >= other.0 {
            self.0 - other.0
        } else {
            other.0 - self.0
        }
    }
}

#[derive(Debug, Error)]
pub enum LsnParseError {
    #[error("Invalid LSN format")]
    InvalidFormat,
    #[error("Parse error: {0}")]
    Parse(#[from] ParseIntError),
}

impl FromStr for Lsn {
    type Err = LsnParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(LsnParseError::InvalidFormat);
        }
        let id = u32::from_str_radix(parts[0], 16)?;
        let offset = u32::from_str_radix(parts[1], 16)?;
        Ok(Lsn(((id as u64) << 32) | (offset as u64)))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplicationState {
    pub client_addr: String,
    pub state: String,
    pub sent_lsn: Lsn,
    pub write_lsn: Lsn,
    pub flush_lsn: Lsn,
    pub replay_lsn: Lsn,
}

impl ReplicationState {
    pub fn byte_lag(&self) -> u64 {
        self.sent_lsn.diff(&self.replay_lsn)
    }
}

pub struct ReplicationMonitor;

impl ReplicationMonitor {
    pub fn parse_pg_stat_replication_row(
        client_addr: &str,
        state: &str,
        sent_lsn: &str,
        write_lsn: &str,
        flush_lsn: &str,
        replay_lsn: &str,
    ) -> Result<ReplicationState, LsnParseError> {
        Ok(ReplicationState {
            client_addr: client_addr.to_string(),
            state: state.to_string(),
            sent_lsn: Lsn::from_str(sent_lsn)?,
            write_lsn: Lsn::from_str(write_lsn)?,
            flush_lsn: Lsn::from_str(flush_lsn)?,
            replay_lsn: Lsn::from_str(replay_lsn)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WalArchiveStatus {
    pub last_archived_wal: Option<String>,
    pub failed_count: u64,
    pub last_failed_wal: Option<String>,
}

impl WalArchiveStatus {
    pub fn is_failing(&self) -> bool {
        self.failed_count > 0 && self.last_failed_wal.is_some()
    }
}

pub struct WalArchiveValidator;

impl WalArchiveValidator {
    pub fn parse_status(
        last_archived_wal: Option<&str>,
        failed_count: &str,
        last_failed_wal: Option<&str>,
    ) -> Result<WalArchiveStatus, ParseIntError> {
        let failed_count_val = if failed_count.trim().is_empty() {
            0
        } else {
            failed_count.trim().parse::<u64>()?
        };

        Ok(WalArchiveStatus {
            last_archived_wal: last_archived_wal.map(|s| s.to_string()),
            failed_count: failed_count_val,
            last_failed_wal: last_failed_wal.map(|s| s.to_string()),
        })
    }
}

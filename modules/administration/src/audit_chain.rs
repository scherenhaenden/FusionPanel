#![forbid(unsafe_code)]

use sha2::{Sha256, Digest};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuditRecord {
    pub id: u64,
    pub tenant_id: String,
    pub timestamp: u64,
    pub action: String,
    pub details: String,
    pub prev_hash: String,
    pub hash: String,
}

#[derive(Error, Debug, PartialEq, Eq)]
pub enum AuditVerificationError {
    #[error("Invalid genesis prev_hash for record {0}")]
    InvalidGenesisHash(u64),
    #[error("Broken link at record {0}")]
    BrokenLink(u64),
    #[error("Modified record at {0}")]
    ModifiedRecord(u64),
}

pub const GENESIS_PREV_HASH: &str = "0000000000000000000000000000000000000000000000000000000000000000";

pub fn compute_hash(
    id: u64,
    tenant_id: &str,
    timestamp: u64,
    action: &str,
    details: &str,
    prev_hash: &str,
) -> String {
    let input = format!("{}:{}:{}:{}:{}:{}", id, tenant_id, timestamp, action, details, prev_hash);
    let mut hasher = Sha256::new();
    hasher.update(input.as_bytes());
    hex::encode(hasher.finalize())
}

#[derive(Debug, Clone, Default)]
pub struct AuditHashChain {
    pub records: Vec<AuditRecord>,
}

impl AuditHashChain {
    pub fn new() -> Self {
        Self { records: Vec::new() }
    }

    pub fn append(&mut self, id: u64, tenant_id: String, timestamp: u64, action: String, details: String) -> AuditRecord {
        let prev_hash = if let Some(last) = self.records.last() {
            last.hash.clone()
        } else {
            GENESIS_PREV_HASH.to_string()
        };

        let hash = compute_hash(id, &tenant_id, timestamp, &action, &details, &prev_hash);

        let record = AuditRecord {
            id,
            tenant_id,
            timestamp,
            action,
            details,
            prev_hash,
            hash,
        };

        self.records.push(record.clone());
        record
    }

    pub fn verify(&self) -> Result<(), AuditVerificationError> {
        verify_chain(&self.records)
    }
}

pub fn verify_chain(records: &[AuditRecord]) -> Result<(), AuditVerificationError> {
    for (i, record) in records.iter().enumerate() {
        if i == 0 {
            if record.prev_hash != GENESIS_PREV_HASH {
                return Err(AuditVerificationError::InvalidGenesisHash(record.id));
            }
        } else {
            let prev_record = &records[i - 1];
            if record.prev_hash != prev_record.hash {
                return Err(AuditVerificationError::BrokenLink(record.id));
            }
        }

        let expected_hash = compute_hash(
            record.id,
            &record.tenant_id,
            record.timestamp,
            &record.action,
            &record.details,
            &record.prev_hash,
        );

        if record.hash != expected_hash {
            return Err(AuditVerificationError::ModifiedRecord(record.id));
        }
    }

    Ok(())
}

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;
use uuid::Uuid;
use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write, Error as IoError};
use std::path::PathBuf;
use std::os::unix::fs::OpenOptionsExt;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditEvent {
    pub id: Uuid,
    pub timestamp: DateTime<Utc>,
    pub actor: String,
    pub action: String,
    pub target: String,
    pub tenant: Option<String>,
    pub ip_address: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditRecord {
    pub event: AuditEvent,
    pub previous_hash: String,
    pub hash: String,
}

#[derive(Error, Debug)]
pub enum AuditError {
    #[error("tamper detected: hash mismatch at event {0}")]
    TamperDetected(Uuid),
    #[error("tamper detected: previous hash mismatch at event {0}")]
    ChainBroken(Uuid),
    #[error("IO error: {0}")]
    Io(#[from] IoError),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
}

pub struct AuditLog {
    records: Vec<AuditRecord>,
    path: Option<PathBuf>,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            records: Vec::new(),
            path: None,
        }
    }

    pub fn with_path(path: impl Into<PathBuf>) -> Result<Self, AuditError> {
        let path = path.into();
        let mut records = Vec::new();

        if path.exists() {
            let file = File::open(&path)?;
            let reader = BufReader::new(file);

            for line in reader.lines() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }
                let record: AuditRecord = serde_json::from_str(&line)?;
                records.push(record);
            }
        }

        Ok(Self {
            records,
            path: Some(path),
        })
    }

    pub fn append(&mut self, event: AuditEvent) -> AuditRecord {
        let previous_hash = self
            .records
            .last()
            .map(|r| r.hash.clone())
            .unwrap_or_else(|| {
                let mut hasher = Sha256::new();
                hasher.update(b"GENESIS");
                hex::encode(hasher.finalize())
            });

        let mut hasher = Sha256::new();
        hasher.update(&previous_hash);

        let event_json = serde_json::to_string(&event).unwrap_or_default();
        hasher.update(&event_json);

        let hash = hex::encode(hasher.finalize());

        let record = AuditRecord {
            event,
            previous_hash,
            hash,
        };

        self.records.push(record.clone());

        if let Some(path) = &self.path {
            // Memory rule: explicit restrictive storage permissions
            if let Ok(mut file) = OpenOptions::new()
                .create(true)
                .append(true)
                .mode(0o600)
                .open(path)
            {
                if let Ok(json_line) = serde_json::to_string(&record) {
                    let _ = writeln!(file, "{}", json_line);
                }
            }
        }

        record
    }

    pub fn verify(&self) -> Result<(), AuditError> {
        let mut expected_prev_hash = {
            let mut hasher = Sha256::new();
            hasher.update(b"GENESIS");
            hex::encode(hasher.finalize())
        };

        for record in &self.records {
            if record.previous_hash != expected_prev_hash {
                return Err(AuditError::ChainBroken(record.event.id));
            }

            let mut hasher = Sha256::new();
            hasher.update(&record.previous_hash);

            let event_json = serde_json::to_string(&record.event).unwrap_or_default();
            hasher.update(&event_json);

            let calculated_hash = hex::encode(hasher.finalize());

            if calculated_hash != record.hash {
                return Err(AuditError::TamperDetected(record.event.id));
            }

            expected_prev_hash = record.hash.clone();
        }

        Ok(())
    }

    pub fn records(&self) -> &[AuditRecord] {
        &self.records
    }

    #[cfg(test)]
    fn tamper_event(&mut self, index: usize, mutated_event: AuditEvent) {
        if let Some(record) = self.records.get_mut(index) {
            record.event = mutated_event;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;
    use tempfile::NamedTempFile;

    fn create_test_event(action: &str) -> AuditEvent {
        AuditEvent {
            id: Uuid::new_v4(),
            timestamp: Utc::now(),
            actor: "admin_user".to_string(),
            action: action.to_string(),
            target: "user_account_123".to_string(),
            tenant: Some("tenant_A".to_string()),
            ip_address: Some("192.168.1.100".to_string()),
            metadata: None,
        }
    }

    #[test]
    fn test_append_and_verify_valid_chain() {
        let mut log = AuditLog::new();

        let event1 = create_test_event("CREATE");
        let event2 = create_test_event("UPDATE");
        let event3 = create_test_event("DELETE");

        log.append(event1.clone());
        log.append(event2.clone());
        log.append(event3.clone());

        assert_eq!(log.records().len(), 3);
        assert!(log.verify().is_ok());
    }

    #[test]
    fn test_tamper_detection_event_data_modified() {
        let mut log = AuditLog::new();

        let event1 = create_test_event("CREATE");
        let event2 = create_test_event("UPDATE");

        log.append(event1);
        log.append(event2.clone());

        // Tamper with the first event
        let mut tampered_event = log.records()[0].event.clone();
        tampered_event.actor = "malicious_user".to_string();

        log.tamper_event(0, tampered_event);

        let verification_result = log.verify();
        assert!(verification_result.is_err());

        if let Err(AuditError::TamperDetected(id)) = verification_result {
            assert_eq!(id, log.records()[0].event.id);
        } else {
            panic!(
                "Expected TamperDetected error, got {:?}",
                verification_result
            );
        }
    }

    #[test]
    fn test_tamper_detection_chain_broken() {
        let mut log = AuditLog::new();

        let event1 = create_test_event("CREATE");
        let event2 = create_test_event("UPDATE");
        let event3 = create_test_event("DELETE");

        log.append(event1);
        log.append(event2);
        log.append(event3);

        // Tamper with the second event to break the chain for the third event
        let mut tampered_event = log.records()[1].event.clone();
        tampered_event.action = "TAMPERED_ACTION".to_string();

        log.tamper_event(1, tampered_event);

        let verification_result = log.verify();
        assert!(verification_result.is_err());

        // We expect tampering to be detected on the second event because its hash no longer matches
        // the data.
        if let Err(AuditError::TamperDetected(id)) = verification_result {
            assert_eq!(id, log.records()[1].event.id);
        } else {
            panic!(
                "Expected TamperDetected error, got {:?}",
                verification_result
            );
        }
    }

    #[test]
    fn test_durable_append_and_load() {
        let temp_file = NamedTempFile::new().unwrap();
        let path = temp_file.path().to_path_buf();

        {
            let mut log = AuditLog::with_path(&path).unwrap();
            log.append(create_test_event("CREATE"));
            log.append(create_test_event("UPDATE"));
        }

        // Re-load
        let log = AuditLog::with_path(&path).unwrap();
        assert_eq!(log.records().len(), 2);
        assert!(log.verify().is_ok());
    }
}

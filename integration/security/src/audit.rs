#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    pub timestamp: DateTime<Utc>,
    pub action: String,
    pub ip_address: Option<String>,
    pub resource: Option<String>,
    pub success: bool,
}

#[derive(Debug, Clone)]
pub struct AuditLog {
    records: Arc<RwLock<Vec<AuditRecord>>>,
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new()
    }
}

impl AuditLog {
    pub fn new() -> Self {
        Self {
            records: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn log(&self, record: AuditRecord) {
        let mut records = self.records.write().await;
        records.push(record);
    }

    pub async fn get_records(&self) -> Vec<AuditRecord> {
        let records = self.records.read().await;
        records.clone()
    }
}

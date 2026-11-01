use crate::domain::{DomainError, Record, Zone};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnssecKey {
    pub key_tag: u16,
    pub algorithm: u8,
    pub digest_type: u8,
    pub digest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DnssecStatus {
    pub enabled: bool,
    pub keys: Vec<DnssecKey>,
}

#[derive(Debug, Error)]
pub enum ProviderError {
    #[error("Authentication failed: {0}")]
    Unauthorized(String),
    #[error("Conflict: {0}")]
    Conflict(String),
    #[error("Dependency unavailable: {0}")]
    DependencyUnavailable(String),
    #[error("Unsupported operation: {0}")]
    Unsupported(String),
    #[error("Operation failed: {0}")]
    OperationFailed(String),
    #[error("Invalid input: {0}")]
    InvalidInput(String),
    #[error("Reconciliation required: {0}")]
    ReconciliationRequired(String),
    #[error("Domain Error: {0}")]
    Domain(#[from] DomainError),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Plan {
    CreateZone(Zone),
    DeleteZone(Uuid),
    UpdateRecords(Uuid, Vec<RecordChange>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RecordChange {
    Add(Record),
    Remove(Record),
    Modify { old: Record, new: Record },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedResult {
    pub plan_id: Uuid,
    pub status: String,
    pub effects: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedState {
    pub zone: Zone,
    pub dnssec: DnssecStatus,
}

pub trait Provider {
    /// Returns the provider name or capability matrix
    fn capabilities(&self) -> Vec<String>;

    /// Plans a change without applying it.
    fn plan_change(&self, plan: Plan) -> Result<Plan, ProviderError>;

    /// Executes a planned change. Must handle idempotency where possible.
    fn apply_change(&self, plan: Plan) -> Result<AppliedResult, ProviderError>;

    /// Observes the current state of a zone.
    fn observe_zone(&self, zone_id: Uuid) -> Result<ObservedState, ProviderError>;

    /// Gets DNSSEC status for a given zone.
    fn get_dnssec_status(&self, zone_id: Uuid) -> Result<DnssecStatus, ProviderError>;
}

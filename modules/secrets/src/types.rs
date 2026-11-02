use serde::{Deserialize, Serialize};
use thiserror::Error;
use zeroize::{Zeroize, ZeroizeOnDrop};

/// Stable error enums for the secrets domain.
#[derive(Debug, Error)]
pub enum SecretError {
    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("Unauthorized: {0}")]
    Unauthorized(String),

    #[error("Conflict: {0}")]
    Conflict(String),

    #[error("Unsupported: {0}")]
    Unsupported(String),

    #[error("Dependency unavailable: {0}")]
    DependencyUnavailable(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Operation failed: {0}")]
    OperationFailed(String),

    #[error("Reconciliation required: {0}")]
    ReconciliationRequired(String),

    #[error("Crypto error: {0}")]
    CryptoError(String),

    #[error("Not found: {0}")]
    NotFound(String),
}

/// A reference to a secret stored in the system.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecretRef {
    /// UUID-compatible string identifier.
    pub id: String,
    pub version: u32,
}

/// A wrapper for sensitive secret material that prevents it from being serialized
/// and zeroizes memory on drop.
#[derive(Clone, PartialEq, Eq, Zeroize, ZeroizeOnDrop)]
pub struct SecretMaterial(Vec<u8>);

impl SecretMaterial {
    pub fn new(data: Vec<u8>) -> Self {
        Self(data)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl std::fmt::Debug for SecretMaterial {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[REDACTED]")
    }
}

/// A request to perform an operation on a secret.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRequest {
    pub operation_id: String,
    pub tenant_id: String,
    pub scope: String,
    pub mode: SecretMode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum SecretMode {
    Create,
    Rotate,
    Read,
    Delete,
    Unsupported(String),
}

/// Distinguish Plan, AppliedResult, and ObservedState.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretPlan {
    pub actions: Vec<String>,
    pub prerequisites: Vec<String>,
    pub conflicts: Vec<String>,
    pub required_permissions: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedResult {
    pub status: String,
    pub resource_refs: Vec<SecretRef>,
    pub events: Vec<String>,
    pub reconciliation_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedState {
    pub secret_id: String,
    pub current_version: u32,
    pub scopes: Vec<String>,
}

/// Explicit response to operations.
#[derive(Debug)]
pub enum SecretResponse {
    Plan(SecretPlan),
    Applied(AppliedResult),
    State(ObservedState),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_redaction() {
        let secret = SecretMaterial::new(b"sensitive_password".to_vec());
        let debug_str = format!("{:?}", secret);
        assert_eq!(debug_str, "[REDACTED]");
        assert!(!debug_str.contains("sensitive_password"));
    }
}

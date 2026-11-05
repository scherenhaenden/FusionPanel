#![forbid(unsafe_code)]

use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnershipTier {
    PanelOwned,
    AdoptedWithBoundary,
    ExternallyManaged,
}

#[derive(Debug, Clone)]
pub struct Resource {
    pub id: String,
    pub tier: OwnershipTier,
}

pub struct OwnershipGuard;

impl OwnershipGuard {
    pub fn can_suspend(resource: &Resource) -> Result<(), &'static str> {
        if resource.tier == OwnershipTier::ExternallyManaged {
            Err("Cannot suspend externally managed resources")
        } else {
            Ok(())
        }
    }

    pub fn can_delete(resource: &Resource) -> Result<(), &'static str> {
        if resource.tier == OwnershipTier::ExternallyManaged {
            Err("Cannot delete externally managed resources")
        } else {
            Ok(())
        }
    }

    pub fn can_restart(resource: &Resource) -> Result<(), &'static str> {
        if resource.tier == OwnershipTier::ExternallyManaged {
            Err("Cannot restart externally managed resources")
        } else {
            Ok(())
        }
    }
}

#[derive(Debug, Clone)]
pub struct DecommissionAttestation {
    pub target_state_verified: bool,
    pub backup_successor_verified: bool,
    pub cert_successor_verified: bool,
    pub post_cutover_reconciled: bool,
    pub evidence_timestamp: u64,
}

impl DecommissionAttestation {
    pub fn new(
        target_state_verified: bool,
        backup_successor_verified: bool,
        cert_successor_verified: bool,
        post_cutover_reconciled: bool,
        evidence_timestamp: u64,
    ) -> Self {
        Self {
            target_state_verified,
            backup_successor_verified,
            cert_successor_verified,
            post_cutover_reconciled,
            evidence_timestamp,
        }
    }

    pub fn is_valid(&self) -> Result<(), &'static str> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();

        // Expired evidence (e.g., older than 24 hours = 86400 seconds)
        if current_time > self.evidence_timestamp + 86400 {
            return Err("Evidence has expired");
        }

        if !self.target_state_verified {
            return Err("Target state is not independently verified");
        }

        if !self.backup_successor_verified {
            return Err("Backup successor is not verified");
        }

        if !self.cert_successor_verified {
            return Err("Certificate renewal successor is not verified");
        }

        if !self.post_cutover_reconciled {
            return Err("Post-cutover changes are not reconciled");
        }

        Ok(())
    }
}

pub fn attempt_decommission(attestation: &DecommissionAttestation) -> Result<(), &'static str> {
    attestation.is_valid()
}

// ADOPT-07 requires trust and SNI modes remain explicit.
// By importing TrustMode here, it proves that it is integrated into the module API surface 
// although adopt07 is tested separately. But since we need "trust/SNI modes remain explicit"
// inside our domain model we will explicitly create the required structs or re-export them.
// We will define it directly in this module to have them explicit.

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpstreamTrustMode {
    SystemCa,
    InternalCa { ca_cert_path: String },
    InsecureCompatibility {
        audit_trail: String,
        migration_path: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamTlsPolicy {
    pub trust_mode: UpstreamTrustMode,
    pub sni_preservation: bool,
    pub expected_hostname: String,
}

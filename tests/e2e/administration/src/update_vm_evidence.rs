#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::Path;

pub const EVIDENCE_SCHEMA_VERSION: &str = "sprint-12-update-vm-v1";

pub const GATE_TWO_RELEASE_UPGRADE: &str = "two_release_upgrade";
pub const GATE_FAILED_HEALTH_ROLLBACK: &str = "failed_health_rollback";
pub const GATE_INTERRUPTED_RECOVERY: &str = "interrupted_recovery";
pub const GATE_SEEDED_STATE_PRESERVED: &str = "seeded_state_preserved";
pub const GATE_DEBIAN_VM_E2E: &str = "debian_vm_e2e";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum GateStatus {
    Pass,
    Fail,
    Blocked,
    NotRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum TeardownStatus {
    Cleaned,
    Failed,
    Skipped,
    NotRun,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactMetadata {
    pub version: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeededEntities {
    pub tenant_id: String,
    pub node_id: String,
    pub site_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateVmEvidenceBundle {
    pub schema_version: String,
    pub recorded_at: DateTime<Utc>,
    pub head_sha: String,
    pub base_sha: String,
    pub old_artifact: ArtifactMetadata,
    pub new_artifact: ArtifactMetadata,
    pub seeded_before: SeededEntities,
    pub seeded_after: SeededEntities,
    pub gates: BTreeMap<String, GateStatus>,
    pub elapsed_ms: u64,
    pub teardown_status: TeardownStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub notes: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvidenceValidationError {
    pub issues: Vec<String>,
}

impl fmt::Display for EvidenceValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "evidence bundle validation failed: {}",
            self.issues.join("; ")
        )
    }
}

impl std::error::Error for EvidenceValidationError {}

impl UpdateVmEvidenceBundle {
    pub fn validate(&self) -> Result<(), EvidenceValidationError> {
        let mut issues = Vec::new();

        if self.schema_version.trim().is_empty() {
            issues.push("schema_version must not be empty".to_string());
        }
        if !is_valid_git_sha(&self.head_sha) {
            issues.push(format!("head_sha is not a valid hex SHA: {}", self.head_sha));
        }
        if !is_valid_git_sha(&self.base_sha) {
            issues.push(format!("base_sha is not a valid hex SHA: {}", self.base_sha));
        }

        validate_artifact("old_artifact", &self.old_artifact, &mut issues);
        validate_artifact("new_artifact", &self.new_artifact, &mut issues);
        if !self.old_artifact.version.is_empty()
            && self.old_artifact.version == self.new_artifact.version
        {
            issues.push(
                "old_artifact.version and new_artifact.version must differ for a two-release upgrade"
                    .to_string(),
            );
        }

        validate_seeded("seeded_before", &self.seeded_before, &mut issues);
        validate_seeded("seeded_after", &self.seeded_after, &mut issues);

        if self.gates.is_empty() {
            issues.push("gates must contain at least one gate status".to_string());
        }

        let upgrade_passed = self.gates.get(GATE_TWO_RELEASE_UPGRADE) == Some(&GateStatus::Pass)
            || self.gates.get(GATE_SEEDED_STATE_PRESERVED) == Some(&GateStatus::Pass);
        if upgrade_passed && self.seeded_before != self.seeded_after {
            issues.push(
                "seeded_before and seeded_after must match when upgrade or state-preservation gates PASS"
                    .to_string(),
            );
        }

        if issues.is_empty() {
            Ok(())
        } else {
            Err(EvidenceValidationError { issues })
        }
    }

    pub fn is_all_passed(&self) -> bool {
        !self.gates.is_empty()
            && self
                .gates
                .values()
                .all(|status| *status == GateStatus::Pass)
            && self.teardown_status == TeardownStatus::Cleaned
    }

    pub fn to_json_pretty(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json_str(raw: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(raw)
    }

    pub fn write_to_path(&self, path: &Path) -> Result<(), String> {
        self.validate().map_err(|err| err.to_string())?;
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                fs::create_dir_all(parent)
                    .map_err(|err| format!("failed to create evidence parent dir: {err}"))?;
            }
        }
        let json = self
            .to_json_pretty()
            .map_err(|err| format!("failed to serialize evidence bundle: {err}"))?;
        fs::write(path, format!("{json}\n"))
            .map_err(|err| format!("failed to write evidence file: {err}"))
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    let mut out = String::with_capacity(digest.len() * 2);
    for byte in digest {
        use fmt::Write as _;
        let _ = write!(&mut out, "{byte:02x}");
    }
    out
}

fn is_valid_git_sha(value: &str) -> bool {
    let len = value.len();
    (7..=64).contains(&len) && value.bytes().all(|b| b.is_ascii_hexdigit())
}

fn is_valid_sha256(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

fn validate_artifact(
    label: &str,
    artifact: &ArtifactMetadata,
    issues: &mut Vec<String>,
) {
    if artifact.version.trim().is_empty() {
        issues.push(format!("{label}.version must not be empty"));
    }
    if !is_valid_sha256(&artifact.sha256) {
        issues.push(format!(
            "{label}.sha256 must be a 64-character lowercase hex SHA-256 digest"
        ));
    }
}

fn validate_seeded(label: &str, seeded: &SeededEntities, issues: &mut Vec<String>) {
    if seeded.tenant_id.trim().is_empty() {
        issues.push(format!("{label}.tenant_id must not be empty"));
    }
    if seeded.node_id.trim().is_empty() {
        issues.push(format!("{label}.node_id must not be empty"));
    }
    if seeded.site_id.trim().is_empty() {
        issues.push(format!("{label}.site_id must not be empty"));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_bundle() -> UpdateVmEvidenceBundle {
        let mut gates = BTreeMap::new();
        gates.insert(GATE_TWO_RELEASE_UPGRADE.to_string(), GateStatus::Pass);
        gates.insert(GATE_FAILED_HEALTH_ROLLBACK.to_string(), GateStatus::Pass);
        gates.insert(GATE_INTERRUPTED_RECOVERY.to_string(), GateStatus::Pass);
        gates.insert(GATE_SEEDED_STATE_PRESERVED.to_string(), GateStatus::Pass);
        gates.insert(GATE_DEBIAN_VM_E2E.to_string(), GateStatus::NotRun);

        UpdateVmEvidenceBundle {
            schema_version: EVIDENCE_SCHEMA_VERSION.to_string(),
            recorded_at: Utc::now(),
            head_sha: "a8861e6".to_string(),
            base_sha: "637dab5".to_string(),
            old_artifact: ArtifactMetadata {
                version: "0.9.0".to_string(),
                sha256: sha256_hex(b"old-artifact"),
            },
            new_artifact: ArtifactMetadata {
                version: "0.10.0".to_string(),
                sha256: sha256_hex(b"new-artifact"),
            },
            seeded_before: SeededEntities {
                tenant_id: "tenant-1".to_string(),
                node_id: "node-1".to_string(),
                site_id: "site-1".to_string(),
            },
            seeded_after: SeededEntities {
                tenant_id: "tenant-1".to_string(),
                node_id: "node-1".to_string(),
                site_id: "site-1".to_string(),
            },
            gates,
            elapsed_ms: 420,
            teardown_status: TeardownStatus::Cleaned,
            notes: None,
        }
    }

    #[test]
    fn evidence_bundle_validates_and_round_trips_json() {
        let bundle = sample_bundle();
        bundle.validate().expect("sample bundle should validate");
        assert!(!bundle.is_all_passed(), "NOT_RUN gate prevents all-passed");

        let json = bundle.to_json_pretty().unwrap();
        assert!(json.contains("\"NOT_RUN\""));
        assert!(json.contains("\"PASS\""));
        assert!(json.contains("\"CLEANED\""));

        let decoded = UpdateVmEvidenceBundle::from_json_str(&json).unwrap();
        assert_eq!(decoded, bundle);
    }

    #[test]
    fn evidence_bundle_rejects_mismatched_seeded_ids_on_pass() {
        let mut bundle = sample_bundle();
        bundle.seeded_after.site_id = "site-mutated".to_string();
        let err = bundle.validate().unwrap_err();
        assert!(err
            .issues
            .iter()
            .any(|issue| issue.contains("seeded_before and seeded_after must match")));
    }

    #[test]
    fn evidence_bundle_rejects_invalid_sha256_and_same_version() {
        let mut bundle = sample_bundle();
        bundle.old_artifact.sha256 = "not-a-sha256".to_string();
        bundle.new_artifact.version = bundle.old_artifact.version.clone();
        let err = bundle.validate().unwrap_err();
        assert!(err.issues.len() >= 2);
    }
}

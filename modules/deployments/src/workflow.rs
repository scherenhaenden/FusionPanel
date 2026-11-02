use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::error::DeploymentError;
use crate::types::{DeploymentId, DeploymentProgressEvent, DeploymentState, LogLevel};

/// Two-person approval record
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApprovalRecord {
    pub deployment_id: DeploymentId,
    pub initiator_identity: String,
    pub approver_identity: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub decided_at: Option<DateTime<Utc>>,
    pub approved: bool,
    pub reason: Option<String>,
}

impl ApprovalRecord {
    pub fn new(
        deployment_id: DeploymentId,
        initiator_identity: impl Into<String>,
    ) -> Result<Self, DeploymentError> {
        let initiator = initiator_identity.into();
        if initiator.trim().is_empty() {
            return Err(DeploymentError::InvalidInput(
                "initiator identity cannot be empty".to_string(),
            ));
        }
        Ok(Self {
            deployment_id,
            initiator_identity: initiator,
            approver_identity: None,
            requested_at: Utc::now(),
            decided_at: None,
            approved: false,
            reason: None,
        })
    }

    /// Approves deployment. Enforces two-person rule: approver cannot be initiator.
    pub fn approve(&mut self, approver_identity: impl Into<String>) -> Result<(), DeploymentError> {
        let approver = approver_identity.into();
        if approver.trim().is_empty() {
            return Err(DeploymentError::InvalidInput(
                "approver identity cannot be empty".to_string(),
            ));
        }

        if approver == self.initiator_identity {
            return Err(DeploymentError::Unauthorized(
                "two-person rule violation: initiator cannot self-approve deployment".to_string(),
            ));
        }

        if self.decided_at.is_some() {
            return Err(DeploymentError::Conflict(
                "deployment approval decision already recorded".to_string(),
            ));
        }

        self.approver_identity = Some(approver);
        self.approved = true;
        self.decided_at = Some(Utc::now());
        Ok(())
    }

    /// Rejects deployment.
    pub fn reject(
        &mut self,
        approver_identity: impl Into<String>,
        reason: impl Into<String>,
    ) -> Result<(), DeploymentError> {
        let approver = approver_identity.into();
        if approver.trim().is_empty() {
            return Err(DeploymentError::InvalidInput(
                "approver identity cannot be empty".to_string(),
            ));
        }

        if self.decided_at.is_some() {
            return Err(DeploymentError::Conflict(
                "deployment approval decision already recorded".to_string(),
            ));
        }

        self.approver_identity = Some(approver);
        self.approved = false;
        self.reason = Some(reason.into());
        self.decided_at = Some(Utc::now());
        Ok(())
    }
}

/// Deployment schedule policy
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleWindow {
    pub not_before: DateTime<Utc>,
    pub not_after: Option<DateTime<Utc>>,
}

impl ScheduleWindow {
    pub fn is_within_window(&self, now: DateTime<Utc>) -> Result<(), DeploymentError> {
        if now < self.not_before {
            return Err(DeploymentError::Conflict(format!(
                "deployment is scheduled for {} which is in the future (current: {})",
                self.not_before, now
            )));
        }
        if let Some(after) = self.not_after {
            if now > after {
                return Err(DeploymentError::Conflict(format!(
                    "deployment schedule window expired at {} (current: {})",
                    after, now
                )));
            }
        }
        Ok(())
    }
}

/// Ephemeral preview environment lifecycle contract
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewEnvironment {
    pub preview_id: String,
    pub commit_sha: String,
    pub domain_name: String,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub active: bool,
}

impl PreviewEnvironment {
    pub fn new(
        preview_id: impl Into<String>,
        commit_sha: impl Into<String>,
        base_domain: &str,
        ttl_seconds: i64,
        now: DateTime<Utc>,
    ) -> Result<Self, DeploymentError> {
        let preview = preview_id.into();
        let commit = commit_sha.into();

        // Validate canonical ASCII DNS name
        let short_sha = if commit.len() >= 7 {
            &commit[..7]
        } else {
            &commit
        };
        let subdomain = format!("prv-{short_sha}");
        let full_domain = format!("{subdomain}.{base_domain}");

        // Validate ASCII domain name rules
        if !full_domain.is_ascii() || full_domain.contains('/') || full_domain.contains(' ') {
            return Err(DeploymentError::InvalidInput(format!(
                "domain '{full_domain}' must be a canonical ASCII DNS name"
            )));
        }

        Ok(Self {
            preview_id: preview,
            commit_sha: commit,
            domain_name: full_domain,
            created_at: now,
            expires_at: now + chrono::Duration::seconds(ttl_seconds),
            active: true,
        })
    }

    pub fn is_expired(&self, now: DateTime<Utc>) -> bool {
        now >= self.expires_at
    }

    pub fn teardown(&mut self) {
        self.active = false;
    }
}

/// Deployment execution history tracker and logger
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeploymentRecord {
    pub deployment_id: DeploymentId,
    pub current_state: DeploymentState,
    pub events: Vec<DeploymentProgressEvent>,
}

impl DeploymentRecord {
    pub fn new(deployment_id: DeploymentId) -> Self {
        let now = Utc::now();
        let initial_event = DeploymentProgressEvent {
            timestamp: now,
            stage: "init".to_string(),
            message: "deployment queued".to_string(),
            level: LogLevel::Info,
        };
        Self {
            deployment_id,
            current_state: DeploymentState::Queued,
            events: vec![initial_event],
        }
    }

    pub fn transition_to(
        &mut self,
        next_state: DeploymentState,
        stage: &str,
        message: &str,
    ) -> Result<(), DeploymentError> {
        if !self.current_state.can_transition_to(next_state) {
            return Err(DeploymentError::Conflict(format!(
                "invalid state transition from {:?} to {:?}",
                self.current_state, next_state
            )));
        }

        self.current_state = next_state;
        let level = match next_state {
            DeploymentState::Failed => LogLevel::Error,
            DeploymentState::Cancelled => LogLevel::Warn,
            _ => LogLevel::Info,
        };

        self.events.push(DeploymentProgressEvent {
            timestamp: Utc::now(),
            stage: stage.to_string(),
            message: message.to_string(),
            level,
        });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_two_person_approval_rejects_self_approval() {
        let dep_id = DeploymentId::new();
        let mut approval = ApprovalRecord::new(dep_id, "alice").unwrap();

        let err = approval.approve("alice");
        assert!(matches!(err, Err(DeploymentError::Unauthorized(_))));

        let ok = approval.approve("bob");
        assert!(ok.is_ok());
        assert!(approval.approved);
        assert_eq!(approval.approver_identity.as_deref(), Some("bob"));

        // Subsequent approval fails as conflict
        let conflict = approval.approve("carol");
        assert!(matches!(conflict, Err(DeploymentError::Conflict(_))));
    }

    #[test]
    fn test_schedule_window() {
        let now = Utc::now();
        let win = ScheduleWindow {
            not_before: now + chrono::Duration::hours(1),
            not_after: Some(now + chrono::Duration::hours(2)),
        };

        // Current time is before window
        assert!(win.is_within_window(now).is_err());

        // In window
        assert!(win
            .is_within_window(now + chrono::Duration::minutes(70))
            .is_ok());

        // Past window
        assert!(win
            .is_within_window(now + chrono::Duration::hours(3))
            .is_err());
    }

    #[test]
    fn test_preview_environment_lifecycle() {
        let now = Utc::now();
        let mut preview = PreviewEnvironment::new(
            "pr-42",
            "3a9a482fb69312597af52ea8162e03876e12ecc8",
            "preview.example.com",
            3600,
            now,
        )
        .unwrap();

        assert_eq!(preview.domain_name, "prv-3a9a482.preview.example.com");
        assert!(!preview.is_expired(now));
        assert!(preview.is_expired(now + chrono::Duration::seconds(3601)));

        preview.teardown();
        assert!(!preview.active);
    }

    #[test]
    fn test_preview_rejects_invalid_ascii() {
        let now = Utc::now();
        let res = PreviewEnvironment::new(
            "pr-42",
            "3a9a482",
            "prëview.example.com", // Non-ASCII
            3600,
            now,
        );
        assert!(matches!(res, Err(DeploymentError::InvalidInput(_))));
    }

    #[test]
    fn test_deployment_history_and_transitions() {
        let mut record = DeploymentRecord::new(DeploymentId::new());
        assert_eq!(record.current_state, DeploymentState::Queued);

        record
            .transition_to(DeploymentState::Building, "builder", "started compile")
            .unwrap();
        assert_eq!(record.current_state, DeploymentState::Building);

        record
            .transition_to(DeploymentState::Deploying, "releaser", "swapping link")
            .unwrap();
        assert_eq!(record.current_state, DeploymentState::Deploying);

        record
            .transition_to(DeploymentState::Active, "releaser", "active and serving")
            .unwrap();
        assert_eq!(record.current_state, DeploymentState::Active);

        // Terminal state cannot transition to building
        let invalid = record.transition_to(DeploymentState::Building, "builder", "retry");
        assert!(invalid.is_err());

        // Active can transition to RolledBack
        let rollback = record.transition_to(DeploymentState::RolledBack, "releaser", "rollback");
        assert!(rollback.is_ok());
    }
}

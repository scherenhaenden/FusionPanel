#![forbid(unsafe_code)]

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NodeIdentity {
    pub node_id: Uuid,
    /// Public key identity or its stable fingerprint; never secret key material.
    pub key_identity: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeOwnership {
    Managed,
    Adopted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub enum NodeStatus {
    #[default]
    Active,
    Draining,
    Offline,
    Degraded,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeCapacity {
    pub cpu: f64,
    pub memory: f64,
    pub active_sites: usize,
    pub max_sites: usize,
}

impl Default for NodeCapacity {
    fn default() -> Self {
        Self {
            cpu: 0.0,
            memory: 0.0,
            active_sites: 0,
            max_sites: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum NodeState {
    Invited {
        invited_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        #[serde(default)]
        enrollment_token: String,
    },
    Enrolled {
        enrolled_at: DateTime<Utc>,
        identity: NodeIdentity,
        ownership: NodeOwnership,
        last_heartbeat: Option<DateTime<Utc>>,
        #[serde(default)]
        status: NodeStatus,
        #[serde(default)]
        capacity: NodeCapacity,
    },
    Revoked {
        revoked_at: DateTime<Utc>,
        reason: String,
        identity: Option<NodeIdentity>,
    },
}

/// An in-memory node lifecycle record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Node {
    id: Uuid,
    state: NodeState,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum StateError {
    #[error("Invitation lifetime must be positive and representable")]
    InvalidInvitationLifetime,
    #[error("Invitation expired")]
    InvitationExpired,
    #[error("Node identity must not be empty")]
    EmptyKeyIdentity,
    #[error("Node already enrolled")]
    AlreadyEnrolled,
    #[error("Node already revoked")]
    AlreadyRevoked,
    #[error("Node not enrolled")]
    NotEnrolled,
    #[error("Invalid enrollment token")]
    InvalidToken,
    #[error("Invalid capacity values")]
    InvalidCapacity,
    #[error("HA and autoscaling are unsupported")]
    UnsupportedHa,
}

impl Node {
    pub fn new_invited(expires_in_hours: i64) -> Result<Self, StateError> {
        if expires_in_hours <= 0 {
            return Err(StateError::InvalidInvitationLifetime);
        }

        let duration = chrono::Duration::try_hours(expires_in_hours)
            .ok_or(StateError::InvalidInvitationLifetime)?;
        let now = Utc::now();
        let expires_at = now
            .checked_add_signed(duration)
            .ok_or(StateError::InvalidInvitationLifetime)?;

        Ok(Self {
            id: Uuid::new_v4(),
            state: NodeState::Invited { enrollment_token: uuid::Uuid::new_v4().to_string(),
                invited_at: now,
                expires_at,
            },
        })
    }


    pub fn enrollment_token(&self) -> Option<&str> {
        match &self.state {
            NodeState::Invited { enrollment_token, .. } => Some(enrollment_token.as_str()),
            _ => None,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn state(&self) -> &NodeState {
        &self.state
    }

    pub fn identity(&self) -> Option<&NodeIdentity> {
        match &self.state {
            NodeState::Enrolled { identity, .. } => Some(identity),
            NodeState::Revoked { identity, .. } => identity.as_ref(),
            NodeState::Invited { .. } => None,
        }
    }

    pub fn enroll(&mut self, node_id: Uuid, key_identity: String, ownership: NodeOwnership, token: String) -> Result<(), StateError> {
        self.enroll_at(Utc::now(), node_id, key_identity, ownership, token)
    }

    fn enroll_at(
        &mut self,
        now: DateTime<Utc>,
        node_id: Uuid,
        key_identity: String,
        ownership: NodeOwnership,
        token: String,
    ) -> Result<(), StateError> {
        if key_identity.trim().is_empty() {
            return Err(StateError::EmptyKeyIdentity);
        }

        match &self.state {
            NodeState::Invited { expires_at, enrollment_token, .. } => {
                if now >= *expires_at {
                    return Err(StateError::InvitationExpired);
                }
                if token != *enrollment_token {
                    return Err(StateError::InvalidToken);
                }
                self.state = NodeState::Enrolled {
                    enrolled_at: now,
                    identity: NodeIdentity {
                        node_id,
                        key_identity,
                    },
                    ownership,
                    last_heartbeat: Some(now),
                    status: NodeStatus::Active,
                    capacity: NodeCapacity::default(),
                };
                Ok(())
            }
            NodeState::Enrolled { .. } => Err(StateError::AlreadyEnrolled),
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
        }
    }

    pub fn revoke(&mut self, reason: String) -> Result<(), StateError> {
        let identity = match &self.state {
            NodeState::Revoked { .. } => return Err(StateError::AlreadyRevoked),
            NodeState::Enrolled { identity, .. } => Some(identity.clone()),
            NodeState::Invited { .. } => None,
        };

        self.state = NodeState::Revoked {
            revoked_at: Utc::now(),
            reason,
            identity,
        };
        Ok(())
    }

    pub fn record_heartbeat(&mut self) -> Result<(), StateError> {
        match &mut self.state {
            NodeState::Enrolled { last_heartbeat, .. } => {
                *last_heartbeat = Some(Utc::now());
                Ok(())
            }
            NodeState::Invited { .. } => Err(StateError::NotEnrolled),
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
        }
    }

    pub fn is_heartbeat_expired(&self, timeout_seconds: i64) -> bool {
        match &self.state {
            NodeState::Enrolled { last_heartbeat: Some(hb), .. } => {
                let timeout_duration = chrono::Duration::try_seconds(timeout_seconds).unwrap_or_else(|| chrono::Duration::seconds(0));
                let now = Utc::now();
                if let Some(expiry) = hb.checked_add_signed(timeout_duration) {
                    now > expiry
                } else {
                    true
                }
            }
            _ => false,
        }
    }

    pub fn reconcile_reconnect(&mut self, now: DateTime<Utc>) -> Result<(), StateError> {
        match &mut self.state {
            NodeState::Enrolled { last_heartbeat, .. } => {
                *last_heartbeat = Some(now);
                Ok(())
            }
            NodeState::Invited { .. } => Err(StateError::NotEnrolled),
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
        }
    }


    pub fn update_status(&mut self, new_status: NodeStatus) -> Result<(), StateError> {
        match &mut self.state {
            NodeState::Enrolled { status, .. } => {
                *status = new_status;
                Ok(())
            }
            NodeState::Invited { .. } => Err(StateError::NotEnrolled),
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
        }
    }

    pub fn report_capacity(&mut self, new_capacity: NodeCapacity) -> Result<(), StateError> {
        if !new_capacity.cpu.is_finite() || new_capacity.cpu < 0.0 || !new_capacity.memory.is_finite() || new_capacity.memory < 0.0 {
             return Err(StateError::InvalidCapacity);
        }

        match &mut self.state {
            NodeState::Enrolled { capacity, .. } => {
                *capacity = new_capacity;
                Ok(())
            }
            NodeState::Invited { .. } => Err(StateError::NotEnrolled),
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
        }
    }

    pub fn evaluate_health(&mut self, timeout_seconds: i64) -> Result<(), StateError> {
        match &mut self.state {
            NodeState::Enrolled { last_heartbeat: Some(hb), status, .. } => {
                let timeout_duration = chrono::Duration::try_seconds(timeout_seconds).unwrap_or_else(|| chrono::Duration::seconds(0));
                let now = Utc::now();
                if let Some(expiry) = hb.checked_add_signed(timeout_duration) {
                    if now > expiry {
                        *status = NodeStatus::Offline;
                    }
                } else {
                    *status = NodeStatus::Offline;
                }
                Ok(())
            }
            NodeState::Enrolled { status, .. } => {
                *status = NodeStatus::Offline;
                Ok(())
            }
            NodeState::Invited { .. } => Err(StateError::NotEnrolled),
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
        }
    }

    pub fn enable_ha(&mut self) -> Result<(), StateError> {
        Err(StateError::UnsupportedHa)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::revocation::RevocationList;

    #[test]
    fn invitation_requires_a_positive_representable_lifetime() {
        assert!(matches!(
            Node::new_invited(0),
            Err(StateError::InvalidInvitationLifetime)
        ));
        assert!(matches!(
            Node::new_invited(-1),
            Err(StateError::InvalidInvitationLifetime)
        ));
        assert!(matches!(
            Node::new_invited(i64::MAX),
            Err(StateError::InvalidInvitationLifetime)
        ));
        assert!(Node::new_invited(24).is_ok());
    }

    #[test]
    fn expired_invitation_cannot_enroll_and_state_is_unchanged() {
        let mut node = Node::new_invited(1).unwrap();
        let expires_at = match node.state() {
            NodeState::Invited { expires_at, .. } => *expires_at,
            _ => unreachable!(),
        };

        assert_eq!(
            node.enroll_at(expires_at, Uuid::new_v4(), "key-identity-123".to_string(), NodeOwnership::Managed, node.enrollment_token().unwrap().to_string()),
            Err(StateError::InvitationExpired)
        );
        assert!(matches!(node.state(), NodeState::Invited { .. }));
    }

    #[test]
    fn empty_key_identity_cannot_enroll() {
        for key_identity in [String::new(), "   ".to_string()] {
            let mut node = Node::new_invited(24).unwrap();
            assert_eq!(
                node.enroll(Uuid::new_v4(), key_identity, NodeOwnership::Managed, node.enrollment_token().unwrap().to_string()),
                Err(StateError::EmptyKeyIdentity)
            );
            assert!(matches!(node.state(), NodeState::Invited { .. }));
        }
    }

    #[test]
    fn revocation_preserves_enrolled_identity_for_revocation_checks() {
        let node_id = Uuid::new_v4();
        let mut node = Node::new_invited(24).unwrap();
        node.enroll(node_id, "key-fingerprint-123".to_string(), NodeOwnership::Managed, node.enrollment_token().unwrap().to_string())
            .unwrap();
        let enrolled_identity = node.identity().unwrap().clone();

        node.revoke("compromised".to_string()).unwrap();

        assert!(matches!(node.state(), NodeState::Revoked { .. }));
        assert_eq!(node.identity(), Some(&enrolled_identity));
        let mut revocation_list = RevocationList::new();
        revocation_list.add(node.identity().unwrap().node_id);
        assert!(revocation_list.is_revoked(&node_id));
        assert_eq!(
            node.revoke("again".to_string()),
            Err(StateError::AlreadyRevoked)
        );
    }

    #[test]
    fn revoked_node_cannot_enroll_again() {
        let mut node = Node::new_invited(24).unwrap();
        node.revoke("invitation withdrawn".to_string()).unwrap();

        assert_eq!(
            node.enroll(Uuid::new_v4(), "key-identity-123".to_string(), NodeOwnership::Managed, "some-token".to_string()),
            Err(StateError::AlreadyRevoked)
        );
    }
}

    #[test]
    fn ha_enable_returns_unsupported() {
        let mut node = Node::new_invited(24).unwrap();
        node.enroll(Uuid::new_v4(), "key-identity-123".to_string(), NodeOwnership::Managed, node.enrollment_token().unwrap().to_string()).unwrap();
        assert_eq!(node.enable_ha(), Err(StateError::UnsupportedHa));
    }

#[cfg(test)]
mod tests2 {
    use super::*;

    #[test]
    fn invalid_enrollment_token_fails() {
        let mut node = Node::new_invited(24).unwrap();
        assert_eq!(
            node.enroll(Uuid::new_v4(), "key".to_string(), NodeOwnership::Managed, "wrong-token".to_string()),
            Err(StateError::InvalidToken)
        );
        assert!(matches!(node.state(), NodeState::Invited { .. }));
    }

    #[test]
    fn can_update_status_and_report_capacity() {
        let mut node = Node::new_invited(24).unwrap();
        node.enroll(Uuid::new_v4(), "key".to_string(), NodeOwnership::Managed, node.enrollment_token().unwrap().to_string()).unwrap();

        assert_eq!(node.update_status(NodeStatus::Draining), Ok(()));
        match node.state() {
            NodeState::Enrolled { status, .. } => assert_eq!(status, &NodeStatus::Draining),
            _ => panic!("Expected Enrolled state"),
        }

        let capacity = NodeCapacity {
            cpu: 0.5,
            memory: 0.6,
            active_sites: 5,
            max_sites: 10,
        };
        assert_eq!(node.report_capacity(capacity.clone()), Ok(()));
        match node.state() {
            NodeState::Enrolled { capacity: c, .. } => assert_eq!(c, &capacity),
            _ => panic!("Expected Enrolled state"),
        }
    }

    #[test]
    fn invalid_capacity_is_rejected() {
        let mut node = Node::new_invited(24).unwrap();
        node.enroll(Uuid::new_v4(), "key".to_string(), NodeOwnership::Managed, node.enrollment_token().unwrap().to_string()).unwrap();

        let mut bad_cpu = NodeCapacity::default();
        bad_cpu.cpu = -1.0;
        assert_eq!(node.report_capacity(bad_cpu), Err(StateError::InvalidCapacity));

        let mut nan_cpu = NodeCapacity::default();
        nan_cpu.cpu = f64::NAN;
        assert_eq!(node.report_capacity(nan_cpu), Err(StateError::InvalidCapacity));
    }

    #[test]
    fn status_and_capacity_fail_on_unenrolled_nodes() {
        let mut node = Node::new_invited(24).unwrap();
        assert_eq!(node.update_status(NodeStatus::Active), Err(StateError::NotEnrolled));
        assert_eq!(node.report_capacity(NodeCapacity::default()), Err(StateError::NotEnrolled));

        node.enroll(Uuid::new_v4(), "key".to_string(), NodeOwnership::Managed, node.enrollment_token().unwrap().to_string()).unwrap();
        node.revoke("reason".to_string()).unwrap();

        assert_eq!(node.update_status(NodeStatus::Active), Err(StateError::AlreadyRevoked));
        assert_eq!(node.report_capacity(NodeCapacity::default()), Err(StateError::AlreadyRevoked));
    }

    #[test]
    fn evaluate_health_marks_offline_when_heartbeat_expired() {
        let mut node = Node::new_invited(24).unwrap();
        node.enroll(Uuid::new_v4(), "key".to_string(), NodeOwnership::Managed, node.enrollment_token().unwrap().to_string()).unwrap();

        // Expire heartbeat manually
        match &mut node.state {
            NodeState::Enrolled { last_heartbeat, .. } => {
                *last_heartbeat = Some(Utc::now() - chrono::Duration::seconds(10));
            }
            _ => panic!("Expected Enrolled state"),
        }

        assert_eq!(node.evaluate_health(5), Ok(()));
        match node.state() {
            NodeState::Enrolled { status, .. } => assert_eq!(status, &NodeStatus::Offline),
            _ => panic!("Expected Enrolled state"),
        }
    }

    #[test]
    fn serialization_backward_and_forward_compatibility() {
        // Old JSON without enrollment_token, status, and capacity
        let old_invited = r#"{"id":"00000000-0000-0000-0000-000000000000","state":{"Invited":{"invited_at":"2023-01-01T00:00:00Z","expires_at":"2023-01-02T00:00:00Z"}}}"#;
        let node: Node = serde_json::from_str(old_invited).unwrap();
        match node.state() {
            NodeState::Invited { enrollment_token, .. } => assert_eq!(enrollment_token, ""),
            _ => panic!("Expected Invited"),
        }

        let old_enrolled = r#"{"id":"00000000-0000-0000-0000-000000000000","state":{"Enrolled":{"enrolled_at":"2023-01-01T00:00:00Z","identity":{"node_id":"00000000-0000-0000-0000-000000000000","key_identity":"key"},"ownership":"Managed","last_heartbeat":null}}}"#;
        let node: Node = serde_json::from_str(old_enrolled).unwrap();
        match node.state() {
            NodeState::Enrolled { status, capacity, .. } => {
                assert_eq!(*status, NodeStatus::Active);
                assert_eq!(*capacity, NodeCapacity::default());
            }
            _ => panic!("Expected Enrolled"),
        }
    }
}

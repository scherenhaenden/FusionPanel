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
pub enum NodeState {
    Invited {
        invited_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    },
    Enrolled {
        enrolled_at: DateTime<Utc>,
        identity: NodeIdentity,
    },
    Revoked {
        revoked_at: DateTime<Utc>,
        reason: String,
        identity: Option<NodeIdentity>,
    },
}

/// An in-memory node lifecycle record.
///
/// Deserialization is deliberately not implemented until persisted state has a
/// validated recovery contract. This prevents callers from reconstructing an
/// arbitrary state and bypassing the checked transition methods.
#[derive(Debug, Clone, Serialize)]
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
            state: NodeState::Invited {
                invited_at: now,
                expires_at,
            },
        })
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

    pub fn enroll(&mut self, node_id: Uuid, key_identity: String) -> Result<(), StateError> {
        self.enroll_at(Utc::now(), node_id, key_identity)
    }

    fn enroll_at(
        &mut self,
        now: DateTime<Utc>,
        node_id: Uuid,
        key_identity: String,
    ) -> Result<(), StateError> {
        if key_identity.trim().is_empty() {
            return Err(StateError::EmptyKeyIdentity);
        }

        match &self.state {
            NodeState::Invited { expires_at, .. } => {
                if now >= *expires_at {
                    return Err(StateError::InvitationExpired);
                }
                self.state = NodeState::Enrolled {
                    enrolled_at: now,
                    identity: NodeIdentity {
                        node_id,
                        key_identity,
                    },
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
            node.enroll_at(expires_at, Uuid::new_v4(), "key-identity-123".to_string()),
            Err(StateError::InvitationExpired)
        );
        assert!(matches!(node.state(), NodeState::Invited { .. }));
    }

    #[test]
    fn empty_key_identity_cannot_enroll() {
        for key_identity in [String::new(), "   ".to_string()] {
            let mut node = Node::new_invited(24).unwrap();
            assert_eq!(
                node.enroll(Uuid::new_v4(), key_identity),
                Err(StateError::EmptyKeyIdentity)
            );
            assert!(matches!(node.state(), NodeState::Invited { .. }));
        }
    }

    #[test]
    fn revocation_preserves_enrolled_identity_for_revocation_checks() {
        let node_id = Uuid::new_v4();
        let mut node = Node::new_invited(24).unwrap();
        node.enroll(node_id, "key-fingerprint-123".to_string())
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
            node.enroll(Uuid::new_v4(), "key-identity-123".to_string()),
            Err(StateError::AlreadyRevoked)
        );
    }
}

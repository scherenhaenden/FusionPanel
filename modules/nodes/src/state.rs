use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum NodeState {
    Invited {
        invited_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
    },
    Enrolled {
        enrolled_at: DateTime<Utc>,
        node_id: Uuid,
        key_identity: Option<String>,
    },
    Revoked {
        revoked_at: DateTime<Utc>,
        reason: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Node {
    pub id: Uuid,
    pub state: NodeState,
}

#[derive(thiserror::Error, Debug)]
pub enum StateError {
    #[error("Invitation expired")]
    InvitationExpired,
    #[error("Node already enrolled")]
    AlreadyEnrolled,
    #[error("Node already revoked")]
    AlreadyRevoked,
}

impl Node {
    pub fn new_invited(expires_in_hours: i64) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            state: NodeState::Invited {
                invited_at: now,
                expires_at: now + chrono::Duration::hours(expires_in_hours),
            },
        }
    }

    pub fn enroll(&mut self, node_id: Uuid, key_identity: String) -> Result<(), StateError> {
        match &self.state {
            NodeState::Invited { expires_at, .. } => {
                if Utc::now() > *expires_at {
                    return Err(StateError::InvitationExpired);
                }
                self.state = NodeState::Enrolled {
                    enrolled_at: Utc::now(),
                    node_id,
                    key_identity: Some(key_identity),
                };
                Ok(())
            }
            NodeState::Enrolled { .. } => Err(StateError::AlreadyEnrolled),
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
        }
    }

    pub fn revoke(&mut self, reason: String) -> Result<(), StateError> {
        match &self.state {
            NodeState::Revoked { .. } => Err(StateError::AlreadyRevoked),
            _ => {
                self.state = NodeState::Revoked {
                    revoked_at: Utc::now(),
                    reason,
                };
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_lifecycle() {
        let mut node = Node::new_invited(24);
        assert!(matches!(node.state, NodeState::Invited { .. }));

        let node_id = Uuid::new_v4();
        assert!(node.enroll(node_id, "key-identity-123".to_string()).is_ok());
        assert!(matches!(node.state, NodeState::Enrolled { .. }));

        assert!(node.revoke("compromised".to_string()).is_ok());
        assert!(matches!(node.state, NodeState::Revoked { .. }));
    }

    #[test]
    fn test_expired_invitation() {
        let mut node = Node::new_invited(-1); // Expired 1 hour ago
        assert!(matches!(
            node.enroll(Uuid::new_v4(), "key-identity-123".to_string()),
            Err(StateError::InvitationExpired)
        ));
    }
}

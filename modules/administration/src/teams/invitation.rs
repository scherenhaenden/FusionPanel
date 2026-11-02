use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};
use subtle::ConstantTimeEq;
use thiserror::Error;
use uuid::Uuid;

#[derive(Error, Debug, PartialEq)]
pub enum InvitationError {
    #[error("Invalid token")]
    InvalidToken,
    #[error("Invitation has expired")]
    Expired,
    #[error("Invitation already accepted")]
    AlreadyAccepted,
    #[error("Invalid role")]
    InvalidRole,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum Role {
    Admin,
    Member,
    Viewer,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Invitation {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub token: String,
    pub role: Role,
    pub expires_at: u64,
    pub accepted: bool,
}

impl Invitation {
    /// Creates a new secure invitation.
    pub fn new(tenant_id: Uuid, role: Role, ttl_seconds: u64) -> Self {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        let expires_at = now + ttl_seconds;

        // Generate a random token
        // In a real implementation this should use a secure CSPRNG
        let token = Uuid::new_v4().to_string();

        Self {
            id: Uuid::new_v4(),
            tenant_id,
            token,
            role,
            expires_at,
            accepted: false,
        }
    }

    /// Accepts an invitation if the token is valid, not expired, and not already accepted.
    pub fn accept(
        &mut self,
        provided_token: &str,
        current_time: u64,
    ) -> Result<Role, InvitationError> {
        if self.accepted {
            return Err(InvitationError::AlreadyAccepted);
        }

        if current_time > self.expires_at {
            return Err(InvitationError::Expired);
        }

        // Secure string comparison to prevent timing attacks
        if self
            .token
            .as_bytes()
            .ct_eq(provided_token.as_bytes())
            .unwrap_u8()
            != 1
        {
            return Err(InvitationError::InvalidToken);
        }

        self.accepted = true;
        Ok(self.role.clone())
    }
}

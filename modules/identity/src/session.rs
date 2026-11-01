use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::error::IdentityError;
use crate::types::{SessionId, TenantId, UserId};

/// Stateful session token and metadata
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Session {
    pub id: SessionId,
    pub user_id: UserId,
    pub tenant_id: TenantId,
    pub created_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub last_accessed_at: DateTime<Utc>,
    pub idle_timeout_seconds: i64,
    pub revoked: bool,
    pub revocation_reason: Option<String>,
}

impl Session {
    pub fn new(
        user_id: UserId,
        tenant_id: TenantId,
        ttl_seconds: i64,
        idle_timeout_seconds: i64,
    ) -> Result<Self, IdentityError> {
        if ttl_seconds <= 0 {
            return Err(IdentityError::InvalidInput {
                message: "Session TTL must be positive".to_string(),
            });
        }
        if idle_timeout_seconds <= 0 {
            return Err(IdentityError::InvalidInput {
                message: "Idle timeout must be positive".to_string(),
            });
        }

        let now = Utc::now();
        let expires_at = now + Duration::seconds(ttl_seconds);

        Ok(Self {
            id: SessionId::new_v4(),
            user_id,
            tenant_id,
            created_at: now,
            expires_at,
            last_accessed_at: now,
            idle_timeout_seconds,
            revoked: false,
            revocation_reason: None,
        })
    }

    /// Validates active session state against current time, enforcing TTL, idle expiry, and revocation.
    pub fn validate_and_touch(&mut self, now: DateTime<Utc>) -> Result<(), IdentityError> {
        if self.revoked {
            return Err(IdentityError::SessionRevoked {
                reason: self
                    .revocation_reason
                    .clone()
                    .unwrap_or_else(|| "Session has been revoked".to_string()),
            });
        }

        if now >= self.expires_at {
            return Err(IdentityError::SessionExpired);
        }

        let idle_limit = self.last_accessed_at + Duration::seconds(self.idle_timeout_seconds);
        if now >= idle_limit {
            return Err(IdentityError::SessionExpired);
        }

        self.last_accessed_at = now;
        Ok(())
    }

    /// Revoke this specific session
    pub fn revoke(&mut self, reason: &str) {
        self.revoked = true;
        self.revocation_reason = Some(reason.to_string());
    }
}

/// Revocation registry for invalidating sessions in-memory or persisted
#[derive(Debug, Default, Clone)]
pub struct RevocationManager {
    revoked_sessions: HashSet<SessionId>,
    revoked_users: HashSet<UserId>,
    revoked_tenants: HashSet<TenantId>,
}

impl RevocationManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn revoke_session(&mut self, session_id: SessionId) {
        self.revoked_sessions.insert(session_id);
    }

    pub fn revoke_user_sessions(&mut self, user_id: UserId) {
        self.revoked_users.insert(user_id);
    }

    pub fn revoke_tenant_sessions(&mut self, tenant_id: TenantId) {
        self.revoked_tenants.insert(tenant_id);
    }

    pub fn is_revoked(&self, session: &Session) -> bool {
        if session.revoked {
            return true;
        }
        if self.revoked_sessions.contains(&session.id) {
            return true;
        }
        if self.revoked_users.contains(&session.user_id) {
            return true;
        }
        if self.revoked_tenants.contains(&session.tenant_id) {
            return true;
        }
        false
    }

    pub fn validate_session(
        &self,
        session: &mut Session,
        now: DateTime<Utc>,
    ) -> Result<(), IdentityError> {
        if self.is_revoked(session) {
            return Err(IdentityError::SessionRevoked {
                reason: "Session has been revoked via revocation manager".to_string(),
            });
        }
        session.validate_and_touch(now)
    }
}

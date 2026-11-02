use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::error::IdentityError;
use crate::types::{SessionId, TenantId, UserId};

/// Upper bounds on session lifetime and idle duration to prevent integer overflow
/// and unrealistic session durations.
pub const MAX_SESSION_TTL_SECONDS: i64 = 365 * 86400; // 1 year
pub const MAX_IDLE_TIMEOUT_SECONDS: i64 = 30 * 86400; // 30 days

/// Stateful session token and metadata.
/// Fields are encapsulated and validated to maintain invariants across construction and deserialization.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SessionRaw")]
pub struct Session {
    id: SessionId,
    user_id: UserId,
    tenant_id: TenantId,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_accessed_at: DateTime<Utc>,
    idle_timeout_seconds: i64,
    revoked: bool,
    revocation_reason: Option<String>,
}

#[derive(Deserialize)]
struct SessionRaw {
    id: SessionId,
    user_id: UserId,
    tenant_id: TenantId,
    created_at: DateTime<Utc>,
    expires_at: DateTime<Utc>,
    last_accessed_at: DateTime<Utc>,
    idle_timeout_seconds: i64,
    revoked: bool,
    revocation_reason: Option<String>,
}

impl TryFrom<SessionRaw> for Session {
    type Error = String;

    fn try_from(raw: SessionRaw) -> Result<Self, Self::Error> {
        let session = Self {
            id: raw.id,
            user_id: raw.user_id,
            tenant_id: raw.tenant_id,
            created_at: raw.created_at,
            expires_at: raw.expires_at,
            last_accessed_at: raw.last_accessed_at,
            idle_timeout_seconds: raw.idle_timeout_seconds,
            revoked: raw.revoked,
            revocation_reason: raw.revocation_reason,
        };
        session.validate().map_err(|e| e.to_string())?;
        Ok(session)
    }
}

impl Session {
    pub fn new(
        user_id: UserId,
        tenant_id: TenantId,
        ttl_seconds: i64,
        idle_timeout_seconds: i64,
    ) -> Result<Self, IdentityError> {
        if ttl_seconds <= 0 || ttl_seconds > MAX_SESSION_TTL_SECONDS {
            return Err(IdentityError::InvalidInput {
                message: format!(
                    "Session TTL must be positive and not exceed {} seconds (given: {})",
                    MAX_SESSION_TTL_SECONDS, ttl_seconds
                ),
            });
        }
        if idle_timeout_seconds <= 0 || idle_timeout_seconds > MAX_IDLE_TIMEOUT_SECONDS {
            return Err(IdentityError::InvalidInput {
                message: format!(
                    "Idle timeout must be positive and not exceed {} seconds (given: {})",
                    MAX_IDLE_TIMEOUT_SECONDS, idle_timeout_seconds
                ),
            });
        }

        let now = Utc::now();
        let duration =
            Duration::try_seconds(ttl_seconds).ok_or_else(|| IdentityError::InvalidInput {
                message: "Session TTL duration calculation overflowed".to_string(),
            })?;

        let expires_at =
            now.checked_add_signed(duration)
                .ok_or_else(|| IdentityError::InvalidInput {
                    message: "Session expiration calculation overflowed".to_string(),
                })?;

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

    pub fn from_persisted(
        id: SessionId,
        user_id: UserId,
        tenant_id: TenantId,
        created_at: DateTime<Utc>,
        expires_at: DateTime<Utc>,
        last_accessed_at: DateTime<Utc>,
        idle_timeout_seconds: i64,
        revoked: bool,
        revocation_reason: Option<String>,
    ) -> Result<Self, IdentityError> {
        let session = Self {
            id,
            user_id,
            tenant_id,
            created_at,
            expires_at,
            last_accessed_at,
            idle_timeout_seconds,
            revoked,
            revocation_reason,
        };
        session.validate()?;
        Ok(session)
    }

    pub fn validate(&self) -> Result<(), IdentityError> {
        if self.expires_at <= self.created_at {
            return Err(IdentityError::InvalidInput {
                message: "Session expires_at must be strictly after created_at".to_string(),
            });
        }
        if self.idle_timeout_seconds <= 0 || self.idle_timeout_seconds > MAX_IDLE_TIMEOUT_SECONDS {
            return Err(IdentityError::InvalidInput {
                message: format!(
                    "Idle timeout must be positive and bounded by {} seconds",
                    MAX_IDLE_TIMEOUT_SECONDS
                ),
            });
        }
        let total_ttl = self
            .expires_at
            .signed_duration_since(self.created_at)
            .num_seconds();
        if total_ttl > MAX_SESSION_TTL_SECONDS {
            return Err(IdentityError::InvalidInput {
                message: format!(
                    "Total session lifetime exceeds maximum bound of {} seconds",
                    MAX_SESSION_TTL_SECONDS
                ),
            });
        }
        if self.last_accessed_at < self.created_at {
            return Err(IdentityError::InvalidInput {
                message: "last_accessed_at cannot precede created_at".to_string(),
            });
        }
        Ok(())
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

        let idle_dur = Duration::try_seconds(self.idle_timeout_seconds).ok_or_else(|| {
            IdentityError::OperationFailed {
                message: "Invalid idle timeout duration".to_string(),
            }
        })?;

        let idle_limit = self
            .last_accessed_at
            .checked_add_signed(idle_dur)
            .ok_or_else(|| IdentityError::OperationFailed {
                message: "Idle limit timestamp overflowed".to_string(),
            })?;

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

    pub fn id(&self) -> SessionId {
        self.id
    }

    pub fn user_id(&self) -> UserId {
        self.user_id
    }

    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn expires_at(&self) -> DateTime<Utc> {
        self.expires_at
    }

    pub fn last_accessed_at(&self) -> DateTime<Utc> {
        self.last_accessed_at
    }

    pub fn idle_timeout_seconds(&self) -> i64 {
        self.idle_timeout_seconds
    }

    pub fn is_revoked(&self) -> bool {
        self.revoked
    }

    pub fn revocation_reason(&self) -> Option<&str> {
        self.revocation_reason.as_deref()
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

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn test_session_creation_valid() {
        let user_id = UserId::new_v4();
        let tenant_id = TenantId::new_v4();
        let session = Session::new(user_id, tenant_id, 3600, 900).unwrap();

        assert_eq!(session.user_id(), user_id);
        assert_eq!(session.tenant_id(), tenant_id);
        assert!(!session.is_revoked());
        assert_eq!(session.idle_timeout_seconds(), 900);
        assert!(session.expires_at() > session.created_at());
    }

    #[test]
    fn test_session_rejects_negative_or_zero_ttl() {
        let user_id = UserId::new_v4();
        let tenant_id = TenantId::new_v4();

        assert!(matches!(
            Session::new(user_id, tenant_id, 0, 900),
            Err(IdentityError::InvalidInput { .. })
        ));
        assert!(matches!(
            Session::new(user_id, tenant_id, -100, 900),
            Err(IdentityError::InvalidInput { .. })
        ));
    }

    #[test]
    fn test_session_rejects_extreme_or_overflow_ttl() {
        let user_id = UserId::new_v4();
        let tenant_id = TenantId::new_v4();

        // Must not panic on i64::MAX or excessive values
        let result = Session::new(user_id, tenant_id, i64::MAX, 900);
        assert!(matches!(result, Err(IdentityError::InvalidInput { .. })));

        let result_huge = Session::new(user_id, tenant_id, MAX_SESSION_TTL_SECONDS + 1, 900);
        assert!(matches!(
            result_huge,
            Err(IdentityError::InvalidInput { .. })
        ));
    }

    #[test]
    fn test_session_rejects_invalid_idle_timeout() {
        let user_id = UserId::new_v4();
        let tenant_id = TenantId::new_v4();

        assert!(matches!(
            Session::new(user_id, tenant_id, 3600, 0),
            Err(IdentityError::InvalidInput { .. })
        ));
        assert!(matches!(
            Session::new(user_id, tenant_id, 3600, -50),
            Err(IdentityError::InvalidInput { .. })
        ));
        assert!(matches!(
            Session::new(user_id, tenant_id, 3600, i64::MAX),
            Err(IdentityError::InvalidInput { .. })
        ));
    }

    #[test]
    fn test_session_touch_and_expiration() {
        let user_id = UserId::new_v4();
        let tenant_id = TenantId::new_v4();
        let mut session = Session::new(user_id, tenant_id, 3600, 600).unwrap();

        let t1 = session.created_at() + Duration::seconds(300);
        assert!(session.validate_and_touch(t1).is_ok());
        assert_eq!(session.last_accessed_at(), t1);

        // Idle timeout exceeded
        let t_idle = t1 + Duration::seconds(601);
        assert_eq!(
            session.validate_and_touch(t_idle),
            Err(IdentityError::SessionExpired)
        );

        // Reset and check hard TTL expiration
        let mut session2 = Session::new(user_id, tenant_id, 3600, 1800).unwrap();
        let t_hard = session2.created_at() + Duration::seconds(3601);
        assert_eq!(
            session2.validate_and_touch(t_hard),
            Err(IdentityError::SessionExpired)
        );
    }

    #[test]
    fn test_session_direct_revocation() {
        let user_id = UserId::new_v4();
        let tenant_id = TenantId::new_v4();
        let mut session = Session::new(user_id, tenant_id, 3600, 900).unwrap();

        session.revoke("Suspected compromise");
        assert!(session.is_revoked());
        assert_eq!(session.revocation_reason(), Some("Suspected compromise"));

        let res = session.validate_and_touch(Utc::now());
        assert!(matches!(res, Err(IdentityError::SessionRevoked { .. })));
    }

    #[test]
    fn test_revocation_manager_cascades() {
        let user1 = UserId::new_v4();
        let user2 = UserId::new_v4();
        let tenant1 = TenantId::new_v4();
        let tenant2 = TenantId::new_v4();

        let s1 = Session::new(user1, tenant1, 3600, 900).unwrap();
        let s2 = Session::new(user2, tenant1, 3600, 900).unwrap();
        let s3 = Session::new(user1, tenant2, 3600, 900).unwrap();

        let mut mgr = RevocationManager::new();

        // Single session revocation
        mgr.revoke_session(s1.id());
        assert!(mgr.is_revoked(&s1));
        assert!(!mgr.is_revoked(&s2));
        assert!(!mgr.is_revoked(&s3));

        // User bulk revocation
        let mut mgr_user = RevocationManager::new();
        mgr_user.revoke_user_sessions(user1);
        assert!(mgr_user.is_revoked(&s1));
        assert!(!mgr_user.is_revoked(&s2));
        assert!(mgr_user.is_revoked(&s3));

        // Tenant bulk revocation
        let mut mgr_tenant = RevocationManager::new();
        mgr_tenant.revoke_tenant_sessions(tenant1);
        assert!(mgr_tenant.is_revoked(&s1));
        assert!(mgr_tenant.is_revoked(&s2));
        assert!(!mgr_tenant.is_revoked(&s3));
    }

    #[test]
    fn test_session_deserialization_validation() {
        // Construct invalid JSON with created_at > expires_at
        let invalid_json = serde_json::json!({
            "id": Uuid::new_v4(),
            "user_id": Uuid::new_v4(),
            "tenant_id": Uuid::new_v4(),
            "created_at": "2026-11-02T12:00:00Z",
            "expires_at": "2026-11-01T12:00:00Z", // In past
            "last_accessed_at": "2026-11-02T12:00:00Z",
            "idle_timeout_seconds": 900,
            "revoked": false,
            "revocation_reason": null
        });

        let res = serde_json::from_value::<Session>(invalid_json);
        assert!(res.is_err());
    }
}

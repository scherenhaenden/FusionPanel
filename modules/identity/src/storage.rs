use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;
use std::str::FromStr;

use crate::error::IdentityError;
use crate::rbac::RoleBinding;
use crate::session::Session;
use crate::types::{SessionId, TenantId, User, UserId, UserStatus};

/// SQLite persistence backend for identity, sessions, and role bindings.
pub struct SqliteIdentityStore {
    conn: Connection,
}

impl SqliteIdentityStore {
    /// Opens or creates an identity database at the specified filesystem path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, IdentityError> {
        let conn = Connection::open(path).map_err(|e| IdentityError::Storage {
            message: format!("Failed to open SQLite database: {}", e),
        })?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    /// Creates an in-memory SQLite database for ephemeral testing.
    pub fn in_memory() -> Result<Self, IdentityError> {
        let conn = Connection::open_in_memory().map_err(|e| IdentityError::Storage {
            message: format!("Failed to open in-memory SQLite database: {}", e),
        })?;
        let store = Self { conn };
        store.init_schema()?;
        Ok(store)
    }

    fn init_schema(&self) -> Result<(), IdentityError> {
        self.conn
            .execute_batch(
                "CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY,
                    username TEXT UNIQUE NOT NULL,
                    email TEXT NOT NULL,
                    status TEXT NOT NULL,
                    password_hash TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    expires_at TEXT NOT NULL,
                    last_accessed_at TEXT NOT NULL,
                    idle_timeout_seconds INTEGER NOT NULL,
                    revoked INTEGER NOT NULL,
                    revocation_reason TEXT
                );

                CREATE TABLE IF NOT EXISTS role_bindings (
                    user_id TEXT NOT NULL,
                    role_json TEXT NOT NULL,
                    scope_json TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS revocations (
                    revocation_type TEXT NOT NULL,
                    target_id TEXT NOT NULL,
                    revoked_at TEXT NOT NULL,
                    reason TEXT,
                    PRIMARY KEY (revocation_type, target_id)
                );",
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Schema initialization failed: {}", e),
            })?;
        Ok(())
    }

    // --- User Operations ---

    pub fn create_user(&self, user: &User) -> Result<(), IdentityError> {
        user.validate()?;
        self.conn
            .execute(
                "INSERT INTO users (id, username, email, status, password_hash, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    user.id().to_string(),
                    user.username(),
                    user.email(),
                    user.status().to_string(),
                    user.password_hash(),
                    user.created_at().to_rfc3339(),
                    user.updated_at().to_rfc3339(),
                ],
            )
            .map_err(|e| match e {
                rusqlite::Error::SqliteFailure(err, _) if err.extended_code == 2067 => {
                    IdentityError::Conflict {
                        message: format!("User with username '{}' already exists", user.username()),
                    }
                }
                _ => IdentityError::Storage {
                    message: format!("Failed to insert user: {}", e),
                },
            })?;
        Ok(())
    }

    pub fn get_user(&self, id: UserId) -> Result<Option<User>, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, username, email, status, password_hash, created_at, updated_at
                 FROM users WHERE id = ?1",
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let user_opt = stmt
            .query_row(params![id.to_string()], |row| {
                let id_str: String = row.get(0)?;
                let username: String = row.get(1)?;
                let email: String = row.get(2)?;
                let status_str: String = row.get(3)?;
                let password_hash: String = row.get(4)?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;

                Ok((
                    id_str,
                    username,
                    email,
                    status_str,
                    password_hash,
                    created_str,
                    updated_str,
                ))
            })
            .optional()
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to fetch user: {}", e),
            })?;

        match user_opt {
            Some((
                id_str,
                username,
                email,
                status_str,
                password_hash,
                created_str,
                updated_str,
            )) => {
                let uid = UserId::from_str(&id_str)?;
                let status = UserStatus::from_str(&status_str)?;
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map_err(|e| IdentityError::Storage {
                        message: format!("Invalid created_at timestamp in store: {}", e),
                    })?
                    .with_timezone(&Utc);
                let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                    .map_err(|e| IdentityError::Storage {
                        message: format!("Invalid updated_at timestamp in store: {}", e),
                    })?
                    .with_timezone(&Utc);

                let user = User::from_persisted(
                    uid,
                    username,
                    email,
                    status,
                    password_hash,
                    created_at,
                    updated_at,
                )?;
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<Option<User>, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, username, email, status, password_hash, created_at, updated_at
                 FROM users WHERE username = ?1",
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let user_opt = stmt
            .query_row(params![username], |row| {
                let id_str: String = row.get(0)?;
                let uname: String = row.get(1)?;
                let email: String = row.get(2)?;
                let status_str: String = row.get(3)?;
                let password_hash: String = row.get(4)?;
                let created_str: String = row.get(5)?;
                let updated_str: String = row.get(6)?;

                Ok((
                    id_str,
                    uname,
                    email,
                    status_str,
                    password_hash,
                    created_str,
                    updated_str,
                ))
            })
            .optional()
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to query user by username: {}", e),
            })?;

        match user_opt {
            Some((id_str, uname, email, status_str, password_hash, created_str, updated_str)) => {
                let uid = UserId::from_str(&id_str)?;
                let status = UserStatus::from_str(&status_str)?;
                let created_at = DateTime::parse_from_rfc3339(&created_str)
                    .map_err(|e| IdentityError::Storage {
                        message: format!("Invalid timestamp: {}", e),
                    })?
                    .with_timezone(&Utc);
                let updated_at = DateTime::parse_from_rfc3339(&updated_str)
                    .map_err(|e| IdentityError::Storage {
                        message: format!("Invalid timestamp: {}", e),
                    })?
                    .with_timezone(&Utc);

                let user = User::from_persisted(
                    uid,
                    uname,
                    email,
                    status,
                    password_hash,
                    created_at,
                    updated_at,
                )?;
                Ok(Some(user))
            }
            None => Ok(None),
        }
    }

    // --- Session Operations ---

    pub fn save_session(&self, session: &Session) -> Result<(), IdentityError> {
        session.validate()?;
        self.conn
            .execute(
                "INSERT OR REPLACE INTO sessions (
                    id, user_id, tenant_id, created_at, expires_at, last_accessed_at,
                    idle_timeout_seconds, revoked, revocation_reason
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
                params![
                    session.id().to_string(),
                    session.user_id().to_string(),
                    session.tenant_id().to_string(),
                    session.created_at().to_rfc3339(),
                    session.expires_at().to_rfc3339(),
                    session.last_accessed_at().to_rfc3339(),
                    session.idle_timeout_seconds(),
                    if session.is_revoked() { 1 } else { 0 },
                    session.revocation_reason(),
                ],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to save session: {}", e),
            })?;
        Ok(())
    }

    pub fn get_session(&self, id: SessionId) -> Result<Option<Session>, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, user_id, tenant_id, created_at, expires_at, last_accessed_at,
                        idle_timeout_seconds, revoked, revocation_reason
                 FROM sessions WHERE id = ?1",
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare statement: {}", e),
            })?;

        let row_opt = stmt
            .query_row(params![id.to_string()], |row| {
                let sid: String = row.get(0)?;
                let uid: String = row.get(1)?;
                let tid: String = row.get(2)?;
                let created: String = row.get(3)?;
                let expires: String = row.get(4)?;
                let last_acc: String = row.get(5)?;
                let idle: i64 = row.get(6)?;
                let revoked_int: i32 = row.get(7)?;
                let reason: Option<String> = row.get(8)?;

                Ok((
                    sid,
                    uid,
                    tid,
                    created,
                    expires,
                    last_acc,
                    idle,
                    revoked_int != 0,
                    reason,
                ))
            })
            .optional()
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to query session: {}", e),
            })?;

        match row_opt {
            Some((sid, uid, tid, created, expires, last_acc, idle, revoked, reason)) => {
                let session_id = SessionId::from_str(&sid)?;
                let user_id = UserId::from_str(&uid)?;
                let tenant_id = TenantId::from_str(&tid)?;
                let created_at = DateTime::parse_from_rfc3339(&created)
                    .map_err(|e| IdentityError::Storage {
                        message: format!("Invalid created_at: {}", e),
                    })?
                    .with_timezone(&Utc);
                let expires_at = DateTime::parse_from_rfc3339(&expires)
                    .map_err(|e| IdentityError::Storage {
                        message: format!("Invalid expires_at: {}", e),
                    })?
                    .with_timezone(&Utc);
                let last_accessed_at = DateTime::parse_from_rfc3339(&last_acc)
                    .map_err(|e| IdentityError::Storage {
                        message: format!("Invalid last_accessed_at: {}", e),
                    })?
                    .with_timezone(&Utc);

                let session = Session::from_persisted(
                    session_id,
                    user_id,
                    tenant_id,
                    created_at,
                    expires_at,
                    last_accessed_at,
                    idle,
                    revoked,
                    reason,
                )?;
                Ok(Some(session))
            }
            None => Ok(None),
        }
    }

    pub fn update_session_touch(
        &self,
        id: SessionId,
        now: DateTime<Utc>,
    ) -> Result<(), IdentityError> {
        let count = self
            .conn
            .execute(
                "UPDATE sessions SET last_accessed_at = ?1 WHERE id = ?2 AND revoked = 0",
                params![now.to_rfc3339(), id.to_string()],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to touch session: {}", e),
            })?;

        if count == 0 {
            // Check if it exists or was revoked
            if let Some(s) = self.get_session(id)? {
                if s.is_revoked() {
                    return Err(IdentityError::SessionRevoked {
                        reason: s
                            .revocation_reason()
                            .unwrap_or("Session revoked")
                            .to_string(),
                    });
                }
            }
            return Err(IdentityError::NotFound {
                resource: "Session".to_string(),
                id: id.to_string(),
            });
        }
        Ok(())
    }

    pub fn revoke_session(&self, id: SessionId, reason: &str) -> Result<(), IdentityError> {
        self.conn
            .execute(
                "UPDATE sessions SET revoked = 1, revocation_reason = ?1 WHERE id = ?2",
                params![reason, id.to_string()],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to revoke session: {}", e),
            })?;
        Ok(())
    }

    pub fn list_tenant_sessions(&self, tenant_id: TenantId) -> Result<Vec<Session>, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                "SELECT id, user_id, tenant_id, created_at, expires_at, last_accessed_at,
                        idle_timeout_seconds, revoked, revocation_reason
                 FROM sessions WHERE tenant_id = ?1",
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare list statement: {}", e),
            })?;

        let rows = stmt
            .query_map(params![tenant_id.to_string()], |row| {
                let sid: String = row.get(0)?;
                let uid: String = row.get(1)?;
                let tid: String = row.get(2)?;
                let created: String = row.get(3)?;
                let expires: String = row.get(4)?;
                let last_acc: String = row.get(5)?;
                let idle: i64 = row.get(6)?;
                let revoked_int: i32 = row.get(7)?;
                let reason: Option<String> = row.get(8)?;

                Ok((
                    sid,
                    uid,
                    tid,
                    created,
                    expires,
                    last_acc,
                    idle,
                    revoked_int != 0,
                    reason,
                ))
            })
            .map_err(|e| IdentityError::Storage {
                message: format!("Query failed: {}", e),
            })?;

        let mut sessions = Vec::new();
        for r in rows {
            let (sid, uid, tid, created, expires, last_acc, idle, revoked, reason) =
                r.map_err(|e| IdentityError::Storage {
                    message: format!("Row error: {}", e),
                })?;

            let session = Session::from_persisted(
                SessionId::from_str(&sid)?,
                UserId::from_str(&uid)?,
                TenantId::from_str(&tid)?,
                DateTime::parse_from_rfc3339(&created)
                    .unwrap()
                    .with_timezone(&Utc),
                DateTime::parse_from_rfc3339(&expires)
                    .unwrap()
                    .with_timezone(&Utc),
                DateTime::parse_from_rfc3339(&last_acc)
                    .unwrap()
                    .with_timezone(&Utc),
                idle,
                revoked,
                reason,
            )?;
            sessions.push(session);
        }
        Ok(sessions)
    }

    // --- Role Binding Operations ---

    pub fn save_role_binding(&self, binding: &RoleBinding) -> Result<(), IdentityError> {
        let role_json =
            serde_json::to_string(&binding.role).map_err(|e| IdentityError::OperationFailed {
                message: format!("Failed to serialize role: {}", e),
            })?;
        let scope_json =
            serde_json::to_string(&binding.scope).map_err(|e| IdentityError::OperationFailed {
                message: format!("Failed to serialize scope: {}", e),
            })?;

        self.conn
            .execute(
                "INSERT INTO role_bindings (user_id, role_json, scope_json) VALUES (?1, ?2, ?3)",
                params![binding.user_id.to_string(), role_json, scope_json],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to insert role binding: {}", e),
            })?;
        Ok(())
    }

    pub fn get_user_role_bindings(
        &self,
        user_id: UserId,
    ) -> Result<Vec<RoleBinding>, IdentityError> {
        let mut stmt = self
            .conn
            .prepare("SELECT role_json, scope_json FROM role_bindings WHERE user_id = ?1")
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let rows = stmt
            .query_map(params![user_id.to_string()], |row| {
                let role_str: String = row.get(0)?;
                let scope_str: String = row.get(1)?;
                Ok((role_str, scope_str))
            })
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to query role bindings: {}", e),
            })?;

        let mut bindings = Vec::new();
        for r in rows {
            let (role_str, scope_str) = r.map_err(|e| IdentityError::Storage {
                message: format!("Row error: {}", e),
            })?;

            let role = serde_json::from_str(&role_str).map_err(|e| IdentityError::Storage {
                message: format!("Failed to deserialize role: {}", e),
            })?;
            let scope = serde_json::from_str(&scope_str).map_err(|e| IdentityError::Storage {
                message: format!("Failed to deserialize scope: {}", e),
            })?;

            bindings.push(RoleBinding {
                user_id,
                role,
                scope,
            });
        }
        Ok(bindings)
    }

    // --- Bulk Revocations ---

    pub fn record_revocation(
        &self,
        revocation_type: &str,
        target_id: &str,
        reason: &str,
    ) -> Result<(), IdentityError> {
        self.conn
            .execute(
                "INSERT OR REPLACE INTO revocations (revocation_type, target_id, revoked_at, reason)
                 VALUES (?1, ?2, ?3, ?4)",
                params![revocation_type, target_id, Utc::now().to_rfc3339(), reason],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to record revocation: {}", e),
            })?;
        Ok(())
    }

    pub fn is_revoked(&self, session: &Session) -> Result<bool, IdentityError> {
        if session.is_revoked() {
            return Ok(true);
        }

        let mut stmt = self
            .conn
            .prepare(
                "SELECT 1 FROM revocations
                 WHERE (revocation_type = 'session' AND target_id = ?1)
                    OR (revocation_type = 'user' AND target_id = ?2)
                    OR (revocation_type = 'tenant' AND target_id = ?3)",
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare revocation check: {}", e),
            })?;

        let exists = stmt
            .exists(params![
                session.id().to_string(),
                session.user_id().to_string(),
                session.tenant_id().to_string(),
            ])
            .map_err(|e| IdentityError::Storage {
                message: format!("Revocation check query failed: {}", e),
            })?;

        Ok(exists)
    }
}

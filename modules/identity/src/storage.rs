use chrono::{DateTime, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::HashSet;
use std::path::Path;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::IdentityError;
use crate::rbac::{Permission, Role, RoleBinding, RoleType, Scope};
use crate::session::Session;
use crate::types::{RoleId, SessionId, SiteId, TenantId, User, UserId, UserStatus};

pub struct IdentityStorage {
    conn: Connection,
}

impl IdentityStorage {
    /// Opens or creates an identity SQLite database at the specified path.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, IdentityError> {
        let conn = Connection::open(path).map_err(|e| IdentityError::Storage {
            message: format!("Failed to open SQLite database: {}", e),
        })?;

        let mut storage = Self { conn };
        storage.initialize_schema()?;
        Ok(storage)
    }

    /// Initializes database schema migrations.
    fn initialize_schema(&mut self) -> Result<(), IdentityError> {
        self.conn
            .execute_batch(
                r#"
                PRAGMA foreign_keys = ON;

                CREATE TABLE IF NOT EXISTS users (
                    id TEXT PRIMARY KEY,
                    username TEXT UNIQUE NOT NULL,
                    email TEXT NOT NULL,
                    status TEXT NOT NULL,
                    password_hash TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS roles (
                    id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    role_type TEXT NOT NULL,
                    permissions TEXT NOT NULL
                );

                CREATE TABLE IF NOT EXISTS role_bindings (
                    user_id TEXT NOT NULL,
                    role_id TEXT NOT NULL,
                    scope_type TEXT NOT NULL,
                    scope_tenant_id TEXT,
                    scope_site_id TEXT,
                    PRIMARY KEY (user_id, role_id, scope_type, scope_tenant_id, scope_site_id),
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE,
                    FOREIGN KEY (role_id) REFERENCES roles(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS sessions (
                    id TEXT PRIMARY KEY,
                    user_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    expires_at TEXT NOT NULL,
                    last_accessed_at TEXT NOT NULL,
                    idle_timeout_seconds INTEGER NOT NULL,
                    revoked INTEGER NOT NULL DEFAULT 0,
                    revocation_reason TEXT,
                    FOREIGN KEY (user_id) REFERENCES users(id) ON DELETE CASCADE
                );

                CREATE TABLE IF NOT EXISTS revocations (
                    id TEXT PRIMARY KEY,
                    target_type TEXT NOT NULL,
                    target_id TEXT NOT NULL,
                    revoked_at TEXT NOT NULL,
                    reason TEXT NOT NULL
                );
                "#,
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to initialize schema: {}", e),
            })?;

        Ok(())
    }

    // --- User Management ---

    pub fn insert_user(&mut self, user: &User) -> Result<(), IdentityError> {
        user.validate()?;

        self.conn
            .execute(
                r#"
                INSERT INTO users (id, username, email, status, password_hash, created_at, updated_at)
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
                "#,
                params![
                    user.id.to_string(),
                    user.username,
                    user.email,
                    user.status.to_string(),
                    user.password_hash,
                    user.created_at.to_rfc3339(),
                    user.updated_at.to_rfc3339(),
                ],
            )
            .map_err(|e| {
                if let rusqlite::Error::SqliteFailure(err, _) = &e {
                    if err.extended_code == 2067 || err.extended_code == 1555 {
                        return IdentityError::Conflict {
                            message: format!("User already exists: {}", user.username),
                        };
                    }
                }
                IdentityError::Storage {
                    message: format!("Failed to insert user: {}", e),
                }
            })?;

        Ok(())
    }

    pub fn get_user(&self, id: UserId) -> Result<User, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, username, email, status, password_hash, created_at, updated_at
                FROM users WHERE id = ?1
                "#,
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let user = stmt
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
                message: format!("Query failed: {}", e),
            })?
            .ok_or_else(|| IdentityError::NotFound {
                resource: "User".to_string(),
                id: id.to_string(),
            })?;

        let user_id = UserId::from_str(&user.0)?;
        let status = UserStatus::from_str(&user.3)?;
        let created_at = DateTime::parse_from_rfc3339(&user.5)
            .map_err(|e| IdentityError::Storage {
                message: format!("Invalid timestamp: {}", e),
            })?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&user.6)
            .map_err(|e| IdentityError::Storage {
                message: format!("Invalid timestamp: {}", e),
            })?
            .with_timezone(&Utc);

        Ok(User {
            id: user_id,
            username: user.1,
            email: user.2,
            status,
            password_hash: user.4,
            created_at,
            updated_at,
        })
    }

    pub fn get_user_by_username(&self, username: &str) -> Result<User, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, username, email, status, password_hash, created_at, updated_at
                FROM users WHERE username = ?1
                "#,
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let user = stmt
            .query_row(params![username], |row| {
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
                message: format!("Query failed: {}", e),
            })?
            .ok_or_else(|| IdentityError::NotFound {
                resource: "User".to_string(),
                id: username.to_string(),
            })?;

        let user_id = UserId::from_str(&user.0)?;
        let status = UserStatus::from_str(&user.3)?;
        let created_at = DateTime::parse_from_rfc3339(&user.5)
            .map_err(|e| IdentityError::Storage {
                message: format!("Invalid timestamp: {}", e),
            })?
            .with_timezone(&Utc);
        let updated_at = DateTime::parse_from_rfc3339(&user.6)
            .map_err(|e| IdentityError::Storage {
                message: format!("Invalid timestamp: {}", e),
            })?
            .with_timezone(&Utc);

        Ok(User {
            id: user_id,
            username: user.1,
            email: user.2,
            status,
            password_hash: user.4,
            created_at,
            updated_at,
        })
    }

    pub fn update_user_status(
        &mut self,
        id: UserId,
        status: UserStatus,
    ) -> Result<(), IdentityError> {
        let now = Utc::now();
        let rows = self
            .conn
            .execute(
                r#"
                UPDATE users SET status = ?1, updated_at = ?2 WHERE id = ?3
                "#,
                params![status.to_string(), now.to_rfc3339(), id.to_string()],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to update user status: {}", e),
            })?;

        if rows == 0 {
            return Err(IdentityError::NotFound {
                resource: "User".to_string(),
                id: id.to_string(),
            });
        }

        // If suspended or revoked, invalidate active sessions
        if status != UserStatus::Active {
            self.revoke_all_user_sessions(id, &format!("User status changed to {}", status))?;
        }

        Ok(())
    }

    // --- Role and RBAC Management ---

    pub fn insert_role(&mut self, role: &Role) -> Result<(), IdentityError> {
        let perms: Vec<String> = role.permissions.iter().map(|p| p.to_string()).collect();
        let perms_json =
            serde_json::to_string(&perms).map_err(|e| IdentityError::OperationFailed {
                message: format!("Failed to serialize permissions: {}", e),
            })?;

        let role_type_str =
            serde_json::to_string(&role.role_type).map_err(|e| IdentityError::OperationFailed {
                message: format!("Failed to serialize role type: {}", e),
            })?;

        self.conn
            .execute(
                r#"
                INSERT INTO roles (id, name, role_type, permissions)
                VALUES (?1, ?2, ?3, ?4)
                "#,
                params![role.id.to_string(), role.name, role_type_str, perms_json],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to insert role: {}", e),
            })?;

        Ok(())
    }

    pub fn get_role(&self, id: RoleId) -> Result<Role, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, name, role_type, permissions
                FROM roles WHERE id = ?1
                "#,
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let (id_str, name, role_type_str, perms_json) = stmt
            .query_row(params![id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .optional()
            .map_err(|e| IdentityError::Storage {
                message: format!("Query failed: {}", e),
            })?
            .ok_or_else(|| IdentityError::NotFound {
                resource: "Role".to_string(),
                id: id.to_string(),
            })?;

        let role_id = RoleId::from_str(&id_str)?;
        let role_type: RoleType =
            serde_json::from_str(&role_type_str).map_err(|e| IdentityError::Storage {
                message: format!("Corrupted role type: {}", e),
            })?;

        let perm_strings: Vec<String> =
            serde_json::from_str(&perms_json).map_err(|e| IdentityError::Storage {
                message: format!("Corrupted permissions: {}", e),
            })?;

        let mut permissions = HashSet::new();
        for p in perm_strings {
            permissions.insert(Permission::from_str(&p)?);
        }

        Ok(Role {
            id: role_id,
            name,
            role_type,
            permissions,
        })
    }

    pub fn bind_role(
        &mut self,
        user_id: UserId,
        role_id: RoleId,
        scope: Scope,
    ) -> Result<(), IdentityError> {
        let (scope_type, scope_tenant_id, scope_site_id) = match scope {
            Scope::Global => ("global", "".to_string(), "".to_string()),
            Scope::Tenant { tenant_id } => ("tenant", tenant_id.to_string(), "".to_string()),
            Scope::Site { tenant_id, site_id } => {
                ("site", tenant_id.to_string(), site_id.to_string())
            }
        };

        self.conn
            .execute(
                r#"
                INSERT INTO role_bindings (user_id, role_id, scope_type, scope_tenant_id, scope_site_id)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![
                    user_id.to_string(),
                    role_id.to_string(),
                    scope_type,
                    scope_tenant_id,
                    scope_site_id
                ],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to insert role binding: {}", e),
            })?;

        Ok(())
    }

    pub fn get_user_bindings(&self, user_id: UserId) -> Result<Vec<RoleBinding>, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT role_id, scope_type, scope_tenant_id, scope_site_id
                FROM role_bindings WHERE user_id = ?1
                "#,
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let rows = stmt
            .query_map(params![user_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| IdentityError::Storage {
                message: format!("Query failed: {}", e),
            })?;

        let mut bindings = Vec::new();
        for r in rows {
            let (role_id_str, scope_type, tenant_str, site_str) =
                r.map_err(|e| IdentityError::Storage {
                    message: format!("Row reading failed: {}", e),
                })?;

            let role_id = RoleId::from_str(&role_id_str)?;
            let role = self.get_role(role_id)?;

            let scope = match scope_type.as_str() {
                "global" => Scope::Global,
                "tenant" => Scope::Tenant {
                    tenant_id: TenantId::from_str(&tenant_str)?,
                },
                "site" => Scope::Site {
                    tenant_id: TenantId::from_str(&tenant_str)?,
                    site_id: SiteId::from_str(&site_str)?,
                },
                _ => {
                    return Err(IdentityError::Storage {
                        message: format!("Unknown scope type: {}", scope_type),
                    })
                }
            };

            bindings.push(RoleBinding {
                user_id,
                role,
                scope,
            });
        }

        Ok(bindings)
    }

    // --- Session & Revocation Management ---

    pub fn insert_session(&mut self, session: &Session) -> Result<(), IdentityError> {
        self.conn
            .execute(
                r#"
                INSERT INTO sessions (
                    id, user_id, tenant_id, created_at, expires_at,
                    last_accessed_at, idle_timeout_seconds, revoked, revocation_reason
                )
                VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
                "#,
                params![
                    session.id.to_string(),
                    session.user_id.to_string(),
                    session.tenant_id.to_string(),
                    session.created_at.to_rfc3339(),
                    session.expires_at.to_rfc3339(),
                    session.last_accessed_at.to_rfc3339(),
                    session.idle_timeout_seconds,
                    if session.revoked { 1 } else { 0 },
                    session.revocation_reason,
                ],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to insert session: {}", e),
            })?;

        Ok(())
    }

    pub fn get_session(&self, id: SessionId) -> Result<Session, IdentityError> {
        let mut stmt = self
            .conn
            .prepare(
                r#"
                SELECT id, user_id, tenant_id, created_at, expires_at,
                       last_accessed_at, idle_timeout_seconds, revoked, revocation_reason
                FROM sessions WHERE id = ?1
                "#,
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to prepare query: {}", e),
            })?;

        let res = stmt
            .query_row(params![id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .optional()
            .map_err(|e| IdentityError::Storage {
                message: format!("Query failed: {}", e),
            })?
            .ok_or_else(|| IdentityError::NotFound {
                resource: "Session".to_string(),
                id: id.to_string(),
            })?;

        let session_id = SessionId::from_str(&res.0)?;
        let user_id = UserId::from_str(&res.1)?;
        let tenant_id = TenantId::from_str(&res.2)?;
        let created_at = DateTime::parse_from_rfc3339(&res.3)
            .map_err(|e| IdentityError::Storage {
                message: format!("Invalid timestamp: {}", e),
            })?
            .with_timezone(&Utc);
        let expires_at = DateTime::parse_from_rfc3339(&res.4)
            .map_err(|e| IdentityError::Storage {
                message: format!("Invalid timestamp: {}", e),
            })?
            .with_timezone(&Utc);
        let last_accessed_at = DateTime::parse_from_rfc3339(&res.5)
            .map_err(|e| IdentityError::Storage {
                message: format!("Invalid timestamp: {}", e),
            })?
            .with_timezone(&Utc);

        Ok(Session {
            id: session_id,
            user_id,
            tenant_id,
            created_at,
            expires_at,
            last_accessed_at,
            idle_timeout_seconds: res.6,
            revoked: res.7 != 0,
            revocation_reason: res.8,
        })
    }

    pub fn update_session_access(
        &mut self,
        id: SessionId,
        last_accessed: DateTime<Utc>,
    ) -> Result<(), IdentityError> {
        self.conn
            .execute(
                r#"
                UPDATE sessions SET last_accessed_at = ?1 WHERE id = ?2
                "#,
                params![last_accessed.to_rfc3339(), id.to_string()],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to touch session: {}", e),
            })?;

        Ok(())
    }

    pub fn revoke_session(&mut self, id: SessionId, reason: &str) -> Result<(), IdentityError> {
        self.conn
            .execute(
                r#"
                UPDATE sessions SET revoked = 1, revocation_reason = ?1 WHERE id = ?2
                "#,
                params![reason, id.to_string()],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to revoke session: {}", e),
            })?;

        self.record_revocation("session", &id.to_string(), reason)?;

        Ok(())
    }

    pub fn revoke_all_user_sessions(
        &mut self,
        user_id: UserId,
        reason: &str,
    ) -> Result<(), IdentityError> {
        self.conn
            .execute(
                r#"
                UPDATE sessions SET revoked = 1, revocation_reason = ?1 WHERE user_id = ?2
                "#,
                params![reason, user_id.to_string()],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to revoke user sessions: {}", e),
            })?;

        self.record_revocation("user", &user_id.to_string(), reason)?;

        Ok(())
    }

    pub fn revoke_all_tenant_sessions(
        &mut self,
        tenant_id: TenantId,
        reason: &str,
    ) -> Result<(), IdentityError> {
        self.conn
            .execute(
                r#"
                UPDATE sessions SET revoked = 1, revocation_reason = ?1 WHERE tenant_id = ?2
                "#,
                params![reason, tenant_id.to_string()],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to revoke tenant sessions: {}", e),
            })?;

        self.record_revocation("tenant", &tenant_id.to_string(), reason)?;

        Ok(())
    }

    fn record_revocation(
        &mut self,
        target_type: &str,
        target_id: &str,
        reason: &str,
    ) -> Result<(), IdentityError> {
        let rev_id = Uuid::new_v4().to_string();
        let now = Utc::now().to_rfc3339();

        self.conn
            .execute(
                r#"
                INSERT INTO revocations (id, target_type, target_id, revoked_at, reason)
                VALUES (?1, ?2, ?3, ?4, ?5)
                "#,
                params![rev_id, target_type, target_id, now, reason],
            )
            .map_err(|e| IdentityError::Storage {
                message: format!("Failed to record revocation tombstone: {}", e),
            })?;

        Ok(())
    }
}

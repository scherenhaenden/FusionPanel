#![forbid(unsafe_code)]

use crate::{ControlStore, StoreError};
use rusqlite::{params, OptionalExtension};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApiTokenRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub scopes: Vec<String>,
    pub created_at: String,
    pub revoked_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditLogEntry {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub actor_id: String,
    pub action: String,
    pub resource_type: String,
    pub resource_id: String,
    pub outcome: String,
    pub detail: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UserMfaRecord {
    pub user_id: Uuid,
    pub totp_secret: String,
    pub enabled: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SecurityRuleRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub rule_type: String,
    pub target: String,
    pub action: String,
    pub description: String,
    pub created_at: String,
}

impl ControlStore {
    #[allow(clippy::too_many_arguments)]
    pub fn create_api_token(
        &mut self,
        token_id: Uuid,
        tenant_id: Uuid,
        user_id: Uuid,
        name: &str,
        token_hash: &str,
        scopes: &[String],
        now: &str,
    ) -> Result<ApiTokenRecord, StoreError> {
        let scopes_json = serde_json::to_string(scopes)?;
        self.conn.execute(
            "INSERT INTO api_tokens (token_id, tenant_id, user_id, name, token_hash, scopes_json, created_at, revoked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![
                token_id.to_string(),
                tenant_id.to_string(),
                user_id.to_string(),
                name,
                token_hash,
                scopes_json,
                now,
            ],
        )?;
        Ok(ApiTokenRecord {
            id: token_id,
            tenant_id,
            user_id,
            name: name.to_string(),
            scopes: scopes.to_vec(),
            created_at: now.to_string(),
            revoked_at: None,
        })
    }

    pub fn list_api_tokens(&mut self, tenant_id: Uuid) -> Result<Vec<ApiTokenRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT token_id, tenant_id, user_id, name, scopes_json, created_at, revoked_at
             FROM api_tokens
             WHERE tenant_id = ?1
             ORDER BY created_at DESC, token_id DESC",
        )?;
        let rows = stmt.query_map([tenant_id.to_string()], row_to_api_token)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn revoke_api_token(
        &mut self,
        tenant_id: Uuid,
        token_id: Uuid,
        now: &str,
    ) -> Result<bool, StoreError> {
        let changed = self.conn.execute(
            "UPDATE api_tokens
             SET revoked_at = COALESCE(revoked_at, ?1)
             WHERE token_id = ?2 AND tenant_id = ?3",
            params![now, token_id.to_string(), tenant_id.to_string()],
        )?;
        Ok(changed > 0)
    }

    pub fn lookup_active_api_token_by_hash(
        &mut self,
        token_hash: &str,
    ) -> Result<Option<ApiTokenRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT token_id, tenant_id, user_id, name, scopes_json, created_at, revoked_at
                 FROM api_tokens
                 WHERE token_hash = ?1 AND revoked_at IS NULL",
                [token_hash],
                row_to_api_token,
            )
            .optional()
            .map_err(StoreError::from)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_audit_log(
        &mut self,
        tenant_id: Uuid,
        actor_id: &str,
        action: &str,
        resource_type: &str,
        resource_id: &str,
        outcome: &str,
        detail: Option<&str>,
        now: &str,
    ) -> Result<AuditLogEntry, StoreError> {
        let id = Uuid::new_v4();
        self.conn.execute(
            "INSERT INTO audit_logs (id, tenant_id, actor_id, action, resource_type, resource_id, outcome, detail, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                id.to_string(),
                tenant_id.to_string(),
                actor_id,
                action,
                resource_type,
                resource_id,
                outcome,
                detail,
                now,
            ],
        )?;
        Ok(AuditLogEntry {
            id,
            tenant_id,
            actor_id: actor_id.to_string(),
            action: action.to_string(),
            resource_type: resource_type.to_string(),
            resource_id: resource_id.to_string(),
            outcome: outcome.to_string(),
            detail: detail.map(str::to_string),
            created_at: now.to_string(),
        })
    }

    pub fn list_audit_logs(
        &mut self,
        tenant_id: Uuid,
        limit: usize,
    ) -> Result<Vec<AuditLogEntry>, StoreError> {
        let clamped = limit.clamp(1, 500) as i64;
        let mut stmt = self.conn.prepare(
            "SELECT id, tenant_id, actor_id, action, resource_type, resource_id, outcome, detail, created_at
             FROM audit_logs
             WHERE tenant_id = ?1
             ORDER BY rowid DESC
             LIMIT ?2",
        )?;
        let rows = stmt.query_map(params![tenant_id.to_string(), clamped], |row| {
            let id_str: String = row.get(0)?;
            let tenant_str: String = row.get(1)?;
            let id = Uuid::parse_str(&id_str).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            let tenant_id = Uuid::parse_str(&tenant_str).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            Ok(AuditLogEntry {
                id,
                tenant_id,
                actor_id: row.get(2)?,
                action: row.get(3)?,
                resource_type: row.get(4)?,
                resource_id: row.get(5)?,
                outcome: row.get(6)?,
                detail: row.get(7)?,
                created_at: row.get(8)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn get_user_mfa(&mut self, user_id: Uuid) -> Result<Option<UserMfaRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT user_id, totp_secret, enabled, created_at, updated_at
                 FROM user_mfa WHERE user_id = ?1",
                [user_id.to_string()],
                |row| {
                    let uid_str: String = row.get(0)?;
                    let uid = Uuid::parse_str(&uid_str).map_err(|err| {
                        rusqlite::Error::FromSqlConversionFailure(
                            0,
                            rusqlite::types::Type::Text,
                            Box::new(err),
                        )
                    })?;
                    let enabled: i64 = row.get(2)?;
                    Ok(UserMfaRecord {
                        user_id: uid,
                        totp_secret: row.get(1)?,
                        enabled: enabled != 0,
                        created_at: row.get(3)?,
                        updated_at: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn upsert_user_mfa_secret(
        &mut self,
        user_id: Uuid,
        totp_secret: &str,
        now: &str,
    ) -> Result<UserMfaRecord, StoreError> {
        self.conn.execute(
            "INSERT INTO user_mfa (user_id, totp_secret, enabled, created_at, updated_at)
             VALUES (?1, ?2, 0, ?3, ?3)
             ON CONFLICT(user_id) DO UPDATE SET totp_secret = excluded.totp_secret, enabled = 0, updated_at = excluded.updated_at",
            params![user_id.to_string(), totp_secret, now],
        )?;
        Ok(UserMfaRecord {
            user_id,
            totp_secret: totp_secret.to_string(),
            enabled: false,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    pub fn enable_user_mfa(&mut self, user_id: Uuid, now: &str) -> Result<bool, StoreError> {
        let changed = self.conn.execute(
            "UPDATE user_mfa SET enabled = 1, updated_at = ?1 WHERE user_id = ?2",
            params![now, user_id.to_string()],
        )?;
        Ok(changed > 0)
    }

    pub fn disable_user_mfa(&mut self, user_id: Uuid) -> Result<bool, StoreError> {
        let changed = self.conn.execute(
            "DELETE FROM user_mfa WHERE user_id = ?1",
            [user_id.to_string()],
        )?;
        Ok(changed > 0)
    }

    pub fn list_security_rules(
        &mut self,
        tenant_id: Uuid,
    ) -> Result<Vec<SecurityRuleRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT rule_id, tenant_id, rule_type, target, action, description, created_at
             FROM security_rules WHERE tenant_id = ?1 ORDER BY created_at DESC, rule_id DESC",
        )?;
        let rows = stmt.query_map([tenant_id.to_string()], |row| {
            let id_str: String = row.get(0)?;
            let tenant_str: String = row.get(1)?;
            let id = Uuid::parse_str(&id_str).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            let tenant_id = Uuid::parse_str(&tenant_str).map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    1,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?;
            Ok(SecurityRuleRecord {
                id,
                tenant_id,
                rule_type: row.get(2)?,
                target: row.get(3)?,
                action: row.get(4)?,
                description: row.get(5)?,
                created_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn create_security_rule(
        &mut self,
        tenant_id: Uuid,
        rule_type: &str,
        target: &str,
        action: &str,
        description: &str,
        now: &str,
    ) -> Result<SecurityRuleRecord, StoreError> {
        let id = Uuid::new_v4();
        self.conn.execute(
            "INSERT INTO security_rules (rule_id, tenant_id, rule_type, target, action, description, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                id.to_string(),
                tenant_id.to_string(),
                rule_type,
                target,
                action,
                description,
                now,
            ],
        )?;
        Ok(SecurityRuleRecord {
            id,
            tenant_id,
            rule_type: rule_type.to_string(),
            target: target.to_string(),
            action: action.to_string(),
            description: description.to_string(),
            created_at: now.to_string(),
        })
    }

    pub fn delete_security_rule(
        &mut self,
        tenant_id: Uuid,
        rule_id: Uuid,
    ) -> Result<bool, StoreError> {
        let changed = self.conn.execute(
            "DELETE FROM security_rules WHERE rule_id = ?1 AND tenant_id = ?2",
            params![rule_id.to_string(), tenant_id.to_string()],
        )?;
        Ok(changed > 0)
    }
}

fn row_to_api_token(row: &rusqlite::Row<'_>) -> Result<ApiTokenRecord, rusqlite::Error> {
    let id_str: String = row.get(0)?;
    let tenant_str: String = row.get(1)?;
    let user_str: String = row.get(2)?;
    let scopes_json: String = row.get(4)?;
    let id = Uuid::parse_str(&id_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let tenant_id = Uuid::parse_str(&tenant_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let user_id = Uuid::parse_str(&user_str).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(err))
    })?;
    let scopes: Vec<String> = serde_json::from_str(&scopes_json).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(err))
    })?;
    Ok(ApiTokenRecord {
        id,
        tenant_id,
        user_id,
        name: row.get(3)?,
        scopes,
        created_at: row.get(5)?,
        revoked_at: row.get(6)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn api_tokens_audit_logs_mfa_and_security_rules_roundtrip() {
        let mut store = ControlStore::new_in_memory().expect("open store");
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let token_id = Uuid::new_v4();

        let created = store
            .create_api_token(
                token_id,
                tenant_id,
                user_id,
                "ci-deploy",
                "hash-123",
                &["sites:read".to_string()],
                "100",
            )
            .expect("create token");
        assert_eq!(created.name, "ci-deploy");
        assert!(store
            .lookup_active_api_token_by_hash("hash-123")
            .unwrap()
            .is_some());
        assert!(store.revoke_api_token(tenant_id, token_id, "101").unwrap());
        assert!(store
            .lookup_active_api_token_by_hash("hash-123")
            .unwrap()
            .is_none());

        store
            .record_audit_log(
                tenant_id,
                "admin",
                "token.create",
                "api_token",
                &token_id.to_string(),
                "success",
                Some("ci-deploy"),
                "102",
            )
            .unwrap();
        let logs = store.list_audit_logs(tenant_id, 10).unwrap();
        assert_eq!(logs.len(), 1);
        assert_eq!(logs[0].action, "token.create");

        store
            .upsert_user_mfa_secret(user_id, "JBSWY3DPEHPK3PXP", "103")
            .unwrap();
        assert!(!store.get_user_mfa(user_id).unwrap().unwrap().enabled);
        assert!(store.enable_user_mfa(user_id, "104").unwrap());
        assert!(store.get_user_mfa(user_id).unwrap().unwrap().enabled);
        assert!(store.disable_user_mfa(user_id).unwrap());
        assert!(store.get_user_mfa(user_id).unwrap().is_none());

        let rule = store
            .create_security_rule(
                tenant_id,
                "firewall",
                "192.0.2.10/32",
                "deny",
                "block scanner",
                "105",
            )
            .unwrap();
        assert_eq!(store.list_security_rules(tenant_id).unwrap().len(), 1);
        assert!(store.delete_security_rule(tenant_id, rule.id).unwrap());
    }
}

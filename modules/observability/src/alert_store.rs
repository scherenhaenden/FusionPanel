#![forbid(unsafe_code)]

use rusqlite::{params, Connection, OptionalExtension};
use secrets::{
    crypto::CryptoEngine,
    types::{SecretError, SecretMaterial},
};
use serde::{Deserialize, Serialize};
use std::{error::Error, fmt, path::Path};
use uuid::Uuid;

const MAX_ALERT_RULES: i64 = 100;
const MAX_DELIVERY_HISTORY: i64 = 1_000;

#[derive(Debug)]
pub enum AlertStoreError {
    Database(rusqlite::Error),
    Crypto(SecretError),
    SecretKeyUnavailable,
    RuleLimitReached,
}

impl fmt::Display for AlertStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("alert persistence failed"),
            Self::Crypto(_) => formatter.write_str("alert secret encryption failed"),
            Self::SecretKeyUnavailable => formatter.write_str("webhook encryption is unavailable"),
            Self::RuleLimitReached => formatter.write_str("alert rule limit reached"),
        }
    }
}

impl Error for AlertStoreError {}

impl From<rusqlite::Error> for AlertStoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}

impl From<SecretError> for AlertStoreError {
    fn from(error: SecretError) -> Self {
        Self::Crypto(error)
    }
}

pub struct AlertStore {
    connection: Connection,
    secret_engine: Option<CryptoEngine>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewAlertRule {
    pub name: String,
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub severity: String,
    pub webhook_url: String,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_enabled() -> bool {
    true
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertRuleRecord {
    pub id: String,
    pub name: String,
    pub metric: String,
    pub operator: String,
    pub threshold: f64,
    pub severity: String,
    pub webhook_url: String,
    pub enabled: bool,
    pub has_secret: bool,
    pub created_at: String,
    pub updated_at: String,
}

pub struct RuleForEvaluation {
    pub rule: AlertRuleRecord,
    pub secret: SecretMaterial,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertStateRecord {
    pub rule_id: String,
    pub node_id: String,
    pub active: bool,
    pub last_value: f64,
    pub started_at: Option<String>,
    pub resolved_at: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlertTransition {
    Triggered,
    Resolved,
}

impl AlertTransition {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Triggered => "triggered",
            Self::Resolved => "resolved",
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertDeliveryRecord {
    pub id: String,
    pub rule_id: String,
    pub node_id: String,
    pub event: String,
    pub status: String,
    pub attempt_count: u32,
    pub http_status: Option<u16>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

impl AlertStore {
    pub fn new(path: &Path, encryption_key: Option<&[u8]>) -> Result<Self, AlertStoreError> {
        let connection = Connection::open(path)?;
        Self::from_connection(connection, encryption_key)
    }

    pub fn in_memory(encryption_key: Option<&[u8]>) -> Result<Self, AlertStoreError> {
        let connection = Connection::open_in_memory()?;
        Self::from_connection(connection, encryption_key)
    }

    fn from_connection(
        connection: Connection,
        encryption_key: Option<&[u8]>,
    ) -> Result<Self, AlertStoreError> {
        connection.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE IF NOT EXISTS alert_rules (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                metric TEXT NOT NULL,
                operator TEXT NOT NULL,
                threshold REAL NOT NULL,
                severity TEXT NOT NULL,
                webhook_url TEXT NOT NULL,
                encrypted_secret BLOB NOT NULL,
                enabled INTEGER NOT NULL,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS alert_states (
                rule_id TEXT NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
                node_id TEXT NOT NULL,
                active INTEGER NOT NULL,
                last_value REAL NOT NULL,
                started_at TEXT,
                resolved_at TEXT,
                updated_at TEXT NOT NULL,
                PRIMARY KEY(rule_id, node_id)
             );
             CREATE TABLE IF NOT EXISTS alert_deliveries (
                id TEXT PRIMARY KEY,
                rule_id TEXT NOT NULL REFERENCES alert_rules(id) ON DELETE CASCADE,
                node_id TEXT NOT NULL,
                event TEXT NOT NULL,
                status TEXT NOT NULL,
                attempt_count INTEGER NOT NULL,
                http_status INTEGER,
                last_error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_alert_deliveries_created
                ON alert_deliveries(created_at DESC, id DESC);",
        )?;
        let secret_engine = encryption_key.map(CryptoEngine::new).transpose()?;
        Ok(Self {
            connection,
            secret_engine,
        })
    }

    pub fn create_rule(
        &mut self,
        rule: &NewAlertRule,
        webhook_secret: &str,
        now: &str,
    ) -> Result<AlertRuleRecord, AlertStoreError> {
        let secret_engine = self
            .secret_engine
            .as_ref()
            .ok_or(AlertStoreError::SecretKeyUnavailable)?;
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM alert_rules", [], |row| row.get(0))?;
        if count >= MAX_ALERT_RULES {
            return Err(AlertStoreError::RuleLimitReached);
        }

        let id = Uuid::new_v4().to_string();
        let secret = SecretMaterial::new(webhook_secret.as_bytes().to_vec());
        let encrypted_secret = secret_engine.encrypt(&secret, id.as_bytes())?;
        self.connection.execute(
            "INSERT INTO alert_rules
             (id, name, metric, operator, threshold, severity, webhook_url, encrypted_secret, enabled, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?10)",
            params![
                id,
                rule.name,
                rule.metric,
                rule.operator,
                rule.threshold,
                rule.severity,
                rule.webhook_url,
                encrypted_secret,
                rule.enabled,
                now,
            ],
        )?;
        Ok(AlertRuleRecord {
            id,
            name: rule.name.clone(),
            metric: rule.metric.clone(),
            operator: rule.operator.clone(),
            threshold: rule.threshold,
            severity: rule.severity.clone(),
            webhook_url: rule.webhook_url.clone(),
            enabled: rule.enabled,
            has_secret: true,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    pub fn list_rules(&self) -> Result<Vec<AlertRuleRecord>, AlertStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, metric, operator, threshold, severity, webhook_url, enabled, created_at, updated_at
             FROM alert_rules ORDER BY created_at, id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AlertRuleRecord {
                id: row.get(0)?,
                name: row.get(1)?,
                metric: row.get(2)?,
                operator: row.get(3)?,
                threshold: row.get(4)?,
                severity: row.get(5)?,
                webhook_url: row.get(6)?,
                enabled: row.get(7)?,
                has_secret: true,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn rules_for_evaluation(&self) -> Result<Vec<RuleForEvaluation>, AlertStoreError> {
        let enabled_count: i64 = self.connection.query_row(
            "SELECT COUNT(*) FROM alert_rules WHERE enabled = 1",
            [],
            |row| row.get(0),
        )?;
        if enabled_count == 0 {
            return Ok(Vec::new());
        }
        let secret_engine = self
            .secret_engine
            .as_ref()
            .ok_or(AlertStoreError::SecretKeyUnavailable)?;
        let mut statement = self.connection.prepare(
            "SELECT id, name, metric, operator, threshold, severity, webhook_url, encrypted_secret, enabled, created_at, updated_at
             FROM alert_rules WHERE enabled = 1 ORDER BY id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                AlertRuleRecord {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    metric: row.get(2)?,
                    operator: row.get(3)?,
                    threshold: row.get(4)?,
                    severity: row.get(5)?,
                    webhook_url: row.get(6)?,
                    enabled: row.get(8)?,
                    has_secret: true,
                    created_at: row.get(9)?,
                    updated_at: row.get(10)?,
                },
                row.get::<_, Vec<u8>>(7)?,
            ))
        })?;
        let mut rules = Vec::new();
        for row in rows {
            let (id, rule, ciphertext) = row?;
            let secret = secret_engine.decrypt(&ciphertext, id.as_bytes())?;
            rules.push(RuleForEvaluation { rule, secret });
        }
        Ok(rules)
    }

    pub fn delete_rule(&mut self, id: &str) -> Result<bool, AlertStoreError> {
        Ok(self
            .connection
            .execute("DELETE FROM alert_rules WHERE id = ?1", params![id])?
            > 0)
    }

    pub fn transition(
        &mut self,
        rule_id: &str,
        node_id: &str,
        active: bool,
        value: f64,
        now: &str,
    ) -> Result<Option<AlertTransition>, AlertStoreError> {
        let previous_active = self
            .connection
            .query_row(
                "SELECT active FROM alert_states WHERE rule_id = ?1 AND node_id = ?2",
                params![rule_id, node_id],
                |row| row.get::<_, bool>(0),
            )
            .optional()?;

        match previous_active {
            None => {
                self.connection.execute(
                    "INSERT INTO alert_states (rule_id, node_id, active, last_value, started_at, resolved_at, updated_at)
                     VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6)",
                    params![rule_id, node_id, active, value, active.then_some(now), now],
                )?;
                Ok(active.then_some(AlertTransition::Triggered))
            }
            Some(was_active) if was_active != active => {
                self.connection.execute(
                    "UPDATE alert_states SET active = ?3, last_value = ?4,
                     started_at = CASE WHEN ?3 = 1 THEN ?5 ELSE started_at END,
                     resolved_at = CASE WHEN ?3 = 1 THEN NULL ELSE ?5 END,
                     updated_at = ?5 WHERE rule_id = ?1 AND node_id = ?2",
                    params![rule_id, node_id, active, value, now],
                )?;
                Ok(Some(if active {
                    AlertTransition::Triggered
                } else {
                    AlertTransition::Resolved
                }))
            }
            Some(_) => {
                self.connection.execute(
                    "UPDATE alert_states SET last_value = ?3, updated_at = ?4 WHERE rule_id = ?1 AND node_id = ?2",
                    params![rule_id, node_id, value, now],
                )?;
                Ok(None)
            }
        }
    }

    pub fn active_states(&self) -> Result<Vec<AlertStateRecord>, AlertStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT rule_id, node_id, active, last_value, started_at, resolved_at, updated_at
             FROM alert_states WHERE active = 1 ORDER BY updated_at DESC, rule_id, node_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok(AlertStateRecord {
                rule_id: row.get(0)?,
                node_id: row.get(1)?,
                active: row.get(2)?,
                last_value: row.get(3)?,
                started_at: row.get(4)?,
                resolved_at: row.get(5)?,
                updated_at: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn create_delivery(
        &mut self,
        rule_id: &str,
        node_id: &str,
        event: &str,
        now: &str,
    ) -> Result<String, AlertStoreError> {
        let id = Uuid::new_v4().to_string();
        self.connection.execute(
            "INSERT INTO alert_deliveries
             (id, rule_id, node_id, event, status, attempt_count, http_status, last_error, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, 'pending', 0, NULL, NULL, ?5, ?5)",
            params![id, rule_id, node_id, event, now],
        )?;
        self.connection.execute(
            "DELETE FROM alert_deliveries WHERE id NOT IN
             (SELECT id FROM alert_deliveries ORDER BY created_at DESC, id DESC LIMIT ?1)",
            params![MAX_DELIVERY_HISTORY],
        )?;
        Ok(id)
    }

    pub fn finish_delivery(
        &mut self,
        id: &str,
        status: &str,
        attempt_count: u32,
        http_status: Option<u16>,
        error_code: Option<&str>,
        now: &str,
    ) -> Result<(), AlertStoreError> {
        self.connection.execute(
            "UPDATE alert_deliveries SET status = ?2, attempt_count = ?3, http_status = ?4,
             last_error = ?5, updated_at = ?6 WHERE id = ?1",
            params![
                id,
                status,
                i64::from(attempt_count),
                http_status.map(i64::from),
                error_code,
                now
            ],
        )?;
        Ok(())
    }

    pub fn deliveries(&self, limit: u32) -> Result<Vec<AlertDeliveryRecord>, AlertStoreError> {
        let limit = limit.clamp(1, 100) as i64;
        let mut statement = self.connection.prepare(
            "SELECT id, rule_id, node_id, event, status, attempt_count, http_status, last_error, created_at, updated_at
             FROM alert_deliveries ORDER BY created_at DESC, id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            Ok(AlertDeliveryRecord {
                id: row.get(0)?,
                rule_id: row.get(1)?,
                node_id: row.get(2)?,
                event: row.get(3)?,
                status: row.get(4)?,
                attempt_count: u32::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
                http_status: row
                    .get::<_, Option<i64>>(6)?
                    .and_then(|status| u16::try_from(status).ok()),
                last_error: row.get(7)?,
                created_at: row.get(8)?,
                updated_at: row.get(9)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrets::crypto::CryptoEngine;

    fn sample_rule() -> NewAlertRule {
        NewAlertRule {
            name: "CPU utilization".to_string(),
            metric: "cpu_usage_percent".to_string(),
            operator: "above".to_string(),
            threshold: 85.0,
            severity: "critical".to_string(),
            webhook_url: "https://alerts.example.test/hook".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn rules_encrypt_secrets_and_only_return_secret_free_views() {
        let key = CryptoEngine::generate_key();
        let secret = hex::encode(CryptoEngine::generate_key());
        let mut store = AlertStore::in_memory(Some(&key)).unwrap();
        let rule = store
            .create_rule(&sample_rule(), &secret, "2026-11-19T10:00:00Z")
            .unwrap();

        assert!(rule.has_secret);
        assert!(!serde_json::to_string(&rule).unwrap().contains(&secret));
        let ciphertext: Vec<u8> = store
            .connection
            .query_row(
                "SELECT encrypted_secret FROM alert_rules WHERE id = ?1",
                params![rule.id],
                |row| row.get(0),
            )
            .unwrap();
        assert_ne!(ciphertext, secret.as_bytes());

        let loaded = store.rules_for_evaluation().unwrap();
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].secret.as_bytes(), secret.as_bytes());
        assert!(!serde_json::to_string(&store.list_rules().unwrap())
            .unwrap()
            .contains(&secret));
    }

    #[test]
    fn threshold_state_emits_only_crossings_and_delivery_history_is_bounded() {
        let key = CryptoEngine::generate_key();
        let secret = hex::encode(CryptoEngine::generate_key());
        let mut store = AlertStore::in_memory(Some(&key)).unwrap();
        let rule = store
            .create_rule(&sample_rule(), &secret, "2026-11-19T10:00:00Z")
            .unwrap();

        assert_eq!(
            store
                .transition(&rule.id, "node-1", false, 70.0, "2026-11-19T10:01:00Z")
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .transition(&rule.id, "node-1", true, 90.0, "2026-11-19T10:02:00Z")
                .unwrap(),
            Some(AlertTransition::Triggered)
        );
        assert_eq!(
            store
                .transition(&rule.id, "node-1", true, 95.0, "2026-11-19T10:03:00Z")
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .transition(&rule.id, "node-1", false, 60.0, "2026-11-19T10:04:00Z")
                .unwrap(),
            Some(AlertTransition::Resolved)
        );
        assert!(store.active_states().unwrap().is_empty());

        let delivery_id = store
            .create_delivery(&rule.id, "node-1", "triggered", "2026-11-19T10:02:00Z")
            .unwrap();
        store
            .finish_delivery(
                &delivery_id,
                "failed",
                3,
                Some(503),
                Some("http_status"),
                "2026-11-19T10:02:06Z",
            )
            .unwrap();
        let deliveries = store.deliveries(100).unwrap();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].status, "failed");
        assert_eq!(deliveries[0].attempt_count, 3);
        assert_eq!(deliveries[0].last_error.as_deref(), Some("http_status"));
    }
}

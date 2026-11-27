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
const MAX_UPTIME_TARGETS: i64 = 100;
const MAX_UPTIME_CHECKS_PER_TARGET: i64 = 120;
const MAX_UPTIME_RESOLVED_INCIDENTS: i64 = 1_000;
const MIN_UPTIME_INTERVAL_SECONDS: u64 = 30;
const MAX_UPTIME_INTERVAL_SECONDS: u64 = 86_400;
const MIN_UPTIME_TIMEOUT_MILLIS: u64 = 250;
const MAX_UPTIME_TIMEOUT_MILLIS: u64 = 10_000;

#[derive(Debug)]
pub enum AlertStoreError {
    Database(rusqlite::Error),
    Crypto(SecretError),
    SecretKeyUnavailable,
    RuleLimitReached,
    UptimeTargetLimitReached,
    UptimeTargetNotFound,
    InvalidUptimeTarget,
}

impl fmt::Display for AlertStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(_) => formatter.write_str("alert persistence failed"),
            Self::Crypto(_) => formatter.write_str("alert secret encryption failed"),
            Self::SecretKeyUnavailable => formatter.write_str("webhook encryption is unavailable"),
            Self::RuleLimitReached => formatter.write_str("alert rule limit reached"),
            Self::UptimeTargetLimitReached => formatter.write_str("uptime target limit reached"),
            Self::UptimeTargetNotFound => formatter.write_str("uptime target not found"),
            Self::InvalidUptimeTarget => formatter.write_str("invalid uptime target"),
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NewUptimeTarget {
    pub name: String,
    pub url: String,
    pub expected_status: u16,
    pub interval_seconds: u64,
    pub timeout_millis: u64,
    pub failure_threshold: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct UptimeTargetRecord {
    pub id: String,
    pub name: String,
    pub url: String,
    pub expected_status: u16,
    pub interval_seconds: u64,
    pub timeout_millis: u64,
    pub failure_threshold: u32,
    pub enabled: bool,
    pub status: String,
    pub consecutive_failures: u32,
    pub last_checked_at: Option<String>,
    pub last_http_status: Option<u16>,
    pub last_latency_ms: Option<u64>,
    pub last_error_code: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UptimeCheckRecord {
    pub target_id: String,
    pub success: bool,
    pub http_status: Option<u16>,
    pub latency_ms: u64,
    pub error_code: Option<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UptimeIncidentRecord {
    pub id: String,
    pub target_id: String,
    pub target_name: String,
    pub state: String,
    pub started_at: String,
    pub recovered_at: Option<String>,
    pub last_error_code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UptimeTransition {
    Down,
    Recovered,
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
                ON alert_deliveries(created_at DESC, id DESC);
             CREATE TABLE IF NOT EXISTS uptime_targets (
                id TEXT PRIMARY KEY,
                name TEXT NOT NULL,
                url TEXT NOT NULL,
                expected_status INTEGER NOT NULL,
                interval_seconds INTEGER NOT NULL,
                timeout_millis INTEGER NOT NULL,
                failure_threshold INTEGER NOT NULL,
                enabled INTEGER NOT NULL DEFAULT 1,
                status TEXT NOT NULL DEFAULT 'pending',
                consecutive_failures INTEGER NOT NULL DEFAULT 0,
                next_check_at_ms INTEGER NOT NULL,
                last_checked_at TEXT,
                last_http_status INTEGER,
                last_latency_ms INTEGER,
                last_error_code TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_uptime_targets_due
                ON uptime_targets(enabled, next_check_at_ms);
             CREATE TABLE IF NOT EXISTS uptime_events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                target_id TEXT NOT NULL REFERENCES uptime_targets(id) ON DELETE CASCADE,
                value REAL NOT NULL, recorded_at TEXT NOT NULL
             );
             CREATE TABLE IF NOT EXISTS uptime_checks (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                target_id TEXT NOT NULL REFERENCES uptime_targets(id) ON DELETE CASCADE,
                success INTEGER NOT NULL,
                http_status INTEGER,
                latency_ms INTEGER NOT NULL,
                error_code TEXT,
                checked_at TEXT NOT NULL
             );
             CREATE INDEX IF NOT EXISTS idx_uptime_checks_target
                ON uptime_checks(target_id, id DESC);
             CREATE TABLE IF NOT EXISTS uptime_incidents (
                id TEXT PRIMARY KEY,
                target_id TEXT NOT NULL REFERENCES uptime_targets(id) ON DELETE CASCADE,
                state TEXT NOT NULL,
                started_at TEXT NOT NULL,
                recovered_at TEXT,
                last_error_code TEXT
             );
             CREATE UNIQUE INDEX IF NOT EXISTS idx_uptime_incidents_active
                ON uptime_incidents(target_id) WHERE state = 'down';
             CREATE INDEX IF NOT EXISTS idx_uptime_incidents_started
                ON uptime_incidents(started_at DESC, id DESC);",
        )?;
        let has_payload: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_table_info('alert_deliveries') WHERE name = 'payload')",
            [], |row| row.get(0),
        )?;
        if !has_payload {
            connection.execute("ALTER TABLE alert_deliveries ADD COLUMN payload TEXT", [])?;
        }
        // Earlier versions did not persist payloads and cannot replay those events.
        connection.execute("UPDATE alert_deliveries SET status = 'failed', last_error = 'missing_persisted_payload' WHERE status = 'pending' AND payload IS NULL", [])?;
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

    /// Persist a state transition and its retryable event as one SQLite transaction.
    pub fn transition_and_enqueue(
        &mut self,
        rule_id: &str,
        node_id: &str,
        active: bool,
        value: f64,
        now: &str,
        payload: &str,
    ) -> Result<Option<String>, AlertStoreError> {
        if let Some(target_id) = node_id.strip_prefix("uptime:") {
            let enabled: bool = self.connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM uptime_targets WHERE id = ?1 AND enabled = 1)",
                params![target_id],
                |row| row.get(0),
            )?;
            if !enabled {
                return Ok(None);
            }
        }
        self.connection
            .execute_batch("SAVEPOINT alert_transition")?;
        let result = (|| {
            let Some(transition) = self.transition(rule_id, node_id, active, value, now)? else {
                return Ok(None);
            };
            let id = self.create_delivery(rule_id, node_id, transition.as_str(), now)?;
            self.connection.execute(
                "UPDATE alert_deliveries SET payload = ?2 WHERE id = ?1",
                params![id, payload],
            )?;
            Ok(Some(id))
        })();
        match result {
            Ok(value) => {
                self.connection.execute_batch("RELEASE alert_transition")?;
                Ok(value)
            }
            Err(error) => {
                self.connection
                    .execute_batch("ROLLBACK TO alert_transition; RELEASE alert_transition")?;
                Err(error)
            }
        }
    }

    pub fn pending_uptime_events(
        &self,
    ) -> Result<Vec<(i64, String, f64, String)>, AlertStoreError> {
        let mut stmt = self
            .connection
            .prepare("SELECT id, target_id, value, recorded_at FROM uptime_events ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn complete_uptime_event(&mut self, id: i64) -> Result<(), AlertStoreError> {
        self.connection
            .execute("DELETE FROM uptime_events WHERE id = ?1", params![id])?;
        Ok(())
    }

    pub fn delivery_pending(&self, id: &str) -> Result<bool, AlertStoreError> {
        Ok(self.connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM alert_deliveries WHERE id = ?1 AND status = 'pending')",
            params![id],
            |row| row.get(0),
        )?)
    }

    pub fn pending_deliveries(&self) -> Result<Vec<(String, String, String)>, AlertStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, rule_id, payload FROM alert_deliveries WHERE status = 'pending' AND payload IS NOT NULL ORDER BY rowid"
        )?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
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
            "DELETE FROM alert_deliveries WHERE status != 'pending' AND id NOT IN
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

    pub fn create_uptime_target(
        &mut self,
        target: &NewUptimeTarget,
        now: &str,
        now_millis: i64,
    ) -> Result<UptimeTargetRecord, AlertStoreError> {
        if target.name.trim().is_empty()
            || target.name.trim().len() > 128
            || target.name.chars().any(char::is_control)
            || target.url.trim().is_empty()
            || target.url.len() > 2_048
            || !(200..=299).contains(&target.expected_status)
            || !(MIN_UPTIME_INTERVAL_SECONDS..=MAX_UPTIME_INTERVAL_SECONDS)
                .contains(&target.interval_seconds)
            || !(MIN_UPTIME_TIMEOUT_MILLIS..=MAX_UPTIME_TIMEOUT_MILLIS)
                .contains(&target.timeout_millis)
            || !(1..=5).contains(&target.failure_threshold)
        {
            return Err(AlertStoreError::InvalidUptimeTarget);
        }
        let count: i64 =
            self.connection
                .query_row("SELECT COUNT(*) FROM uptime_targets", [], |row| row.get(0))?;
        if count >= MAX_UPTIME_TARGETS {
            return Err(AlertStoreError::UptimeTargetLimitReached);
        }

        let id = Uuid::new_v4().to_string();
        self.connection.execute(
            "INSERT INTO uptime_targets
             (id, name, url, expected_status, interval_seconds, timeout_millis, failure_threshold,
              next_check_at_ms, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                id,
                target.name,
                target.url,
                i64::from(target.expected_status),
                i64::try_from(target.interval_seconds).unwrap_or(i64::MAX),
                i64::try_from(target.timeout_millis).unwrap_or(i64::MAX),
                i64::from(target.failure_threshold),
                now_millis,
                now,
            ],
        )?;
        Ok(UptimeTargetRecord {
            id,
            name: target.name.clone(),
            url: target.url.clone(),
            expected_status: target.expected_status,
            interval_seconds: target.interval_seconds,
            timeout_millis: target.timeout_millis,
            failure_threshold: target.failure_threshold,
            enabled: true,
            status: "pending".to_string(),
            consecutive_failures: 0,
            last_checked_at: None,
            last_http_status: None,
            last_latency_ms: None,
            last_error_code: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        })
    }

    pub fn uptime_targets(&self) -> Result<Vec<UptimeTargetRecord>, AlertStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, url, expected_status, interval_seconds, timeout_millis,
                    failure_threshold, enabled, status, consecutive_failures, last_checked_at,
                    last_http_status, last_latency_ms, last_error_code, created_at, updated_at
             FROM uptime_targets ORDER BY created_at, id",
        )?;
        let rows = statement.query_map([], uptime_target_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn disable_uptime_target(
        &mut self,
        id: &str,
        disabled_at: &str,
    ) -> Result<bool, AlertStoreError> {
        let transaction = self.connection.transaction()?;
        let exists = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM uptime_targets WHERE id = ?1)",
            params![id],
            |row| row.get::<_, bool>(0),
        )?;
        if !exists {
            return Ok(false);
        }
        transaction.execute(
            "UPDATE uptime_targets SET enabled = 0, status = 'disabled', updated_at = ?2
             WHERE id = ?1",
            params![id, disabled_at],
        )?;
        let node_id = format!("uptime:{id}");
        transaction.execute(
            "DELETE FROM uptime_events WHERE target_id = ?1",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM alert_states WHERE node_id = ?1",
            params![node_id],
        )?;
        transaction.execute(
            "DELETE FROM alert_deliveries WHERE node_id = ?1 AND status = 'pending'",
            params![node_id],
        )?;
        transaction.execute(
            "UPDATE uptime_incidents SET state = 'cancelled', last_error_code = 'target_disabled'
             WHERE target_id = ?1 AND state = 'down'",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM uptime_incidents WHERE state IN ('recovered', 'cancelled') AND id NOT IN
             (SELECT id FROM uptime_incidents WHERE state IN ('recovered', 'cancelled')
              ORDER BY started_at DESC, id DESC LIMIT ?1)",
            params![MAX_UPTIME_RESOLVED_INCIDENTS],
        )?;
        transaction.commit()?;
        Ok(true)
    }

    pub fn delete_uptime_target(&mut self, id: &str) -> Result<bool, AlertStoreError> {
        let transaction = self.connection.transaction()?;
        let deleted =
            transaction.execute("DELETE FROM uptime_targets WHERE id = ?1", params![id])? > 0;
        if deleted {
            let node_id = format!("uptime:{id}");
            transaction.execute(
                "DELETE FROM alert_states WHERE node_id = ?1",
                params![node_id],
            )?;
            transaction.execute(
                "DELETE FROM alert_deliveries WHERE node_id = ?1 AND status = 'pending'",
                params![node_id],
            )?;
        }
        transaction.commit()?;
        Ok(deleted)
    }

    pub fn due_uptime_targets(
        &self,
        now_millis: i64,
    ) -> Result<Vec<UptimeTargetRecord>, AlertStoreError> {
        let mut statement = self.connection.prepare(
            "SELECT id, name, url, expected_status, interval_seconds, timeout_millis,
                    failure_threshold, enabled, status, consecutive_failures, last_checked_at,
                    last_http_status, last_latency_ms, last_error_code, created_at, updated_at
             FROM uptime_targets
             WHERE enabled = 1 AND next_check_at_ms <= ?1
             ORDER BY next_check_at_ms, id",
        )?;
        let rows = statement.query_map(params![now_millis], uptime_target_from_row)?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_uptime_check(
        &mut self,
        target_id: &str,
        success: bool,
        http_status: Option<u16>,
        latency_millis: u64,
        error_code: Option<&str>,
        checked_at: &str,
        checked_at_millis: i64,
    ) -> Result<Option<UptimeTransition>, AlertStoreError> {
        let transaction = self.connection.transaction()?;
        let target = transaction
            .query_row(
                "SELECT status, consecutive_failures, failure_threshold, interval_seconds
                 FROM uptime_targets WHERE id = ?1 AND enabled = 1",
                params![target_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, i64>(2)?,
                        row.get::<_, i64>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(AlertStoreError::UptimeTargetNotFound)?;
        let previous_status = target.0;
        let failures = if success {
            0
        } else {
            target.1.saturating_add(1)
        };
        let status = if success {
            "up"
        } else if failures >= target.2 {
            "down"
        } else {
            "failing"
        };
        let transition = match (previous_status == "down", status == "down") {
            (false, true) => Some(UptimeTransition::Down),
            (true, false) if success => Some(UptimeTransition::Recovered),
            _ => None,
        };

        transaction.execute(
            "INSERT INTO uptime_checks
             (target_id, success, http_status, latency_ms, error_code, checked_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                target_id,
                success,
                http_status.map(i64::from),
                i64::try_from(latency_millis).unwrap_or(i64::MAX),
                error_code,
                checked_at,
            ],
        )?;
        transaction.execute(
            "DELETE FROM uptime_checks WHERE target_id = ?1 AND id NOT IN
             (SELECT id FROM uptime_checks WHERE target_id = ?1 ORDER BY id DESC LIMIT ?2)",
            params![target_id, MAX_UPTIME_CHECKS_PER_TARGET],
        )?;
        transaction.execute(
            "UPDATE uptime_targets SET status = ?2, consecutive_failures = ?3,
             last_checked_at = ?4, last_http_status = ?5, last_latency_ms = ?6,
             last_error_code = ?7, next_check_at_ms = ?8, updated_at = ?4
             WHERE id = ?1",
            params![
                target_id,
                status,
                failures,
                checked_at,
                http_status.map(i64::from),
                i64::try_from(latency_millis).unwrap_or(i64::MAX),
                error_code,
                checked_at_millis.saturating_add(target.3.saturating_mul(1_000)),
            ],
        )?;

        match transition {
            Some(UptimeTransition::Down) => {
                transaction.execute(
                    "INSERT INTO uptime_incidents
                     (id, target_id, state, started_at, recovered_at, last_error_code)
                     VALUES (?1, ?2, 'down', ?3, NULL, ?4)",
                    params![
                        Uuid::new_v4().to_string(),
                        target_id,
                        checked_at,
                        error_code
                    ],
                )?;
            }
            Some(UptimeTransition::Recovered) => {
                transaction.execute(
                    "UPDATE uptime_incidents SET state = 'recovered', recovered_at = ?2
                     WHERE target_id = ?1 AND state = 'down'",
                    params![target_id, checked_at],
                )?;
                transaction.execute(
                    "DELETE FROM uptime_incidents
                     WHERE state IN ('recovered', 'cancelled') AND id NOT IN
                     (SELECT id FROM uptime_incidents WHERE state IN ('recovered', 'cancelled')
                      ORDER BY started_at DESC, id DESC LIMIT ?1)",
                    params![MAX_UPTIME_RESOLVED_INCIDENTS],
                )?;
            }
            None => {}
        }
        if let Some(event) = transition {
            let value = match event {
                UptimeTransition::Down => 0.0,
                UptimeTransition::Recovered => 100.0,
            };
            transaction.execute(
                "INSERT INTO uptime_events (target_id, value, recorded_at) VALUES (?1, ?2, ?3)",
                params![target_id, value, checked_at],
            )?;
        }
        transaction.commit()?;
        Ok(transition)
    }

    pub fn uptime_checks(
        &self,
        target_id: &str,
        limit: u32,
    ) -> Result<Vec<UptimeCheckRecord>, AlertStoreError> {
        let limit = i64::from(limit.clamp(1, 120));
        let mut statement = self.connection.prepare(
            "SELECT target_id, success, http_status, latency_ms, error_code, checked_at
             FROM uptime_checks WHERE target_id = ?1 ORDER BY id DESC LIMIT ?2",
        )?;
        let rows = statement.query_map(params![target_id, limit], |row| {
            Ok(UptimeCheckRecord {
                target_id: row.get(0)?,
                success: row.get(1)?,
                http_status: row
                    .get::<_, Option<i64>>(2)?
                    .and_then(|value| u16::try_from(value).ok()),
                latency_ms: u64::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
                error_code: row.get(4)?,
                checked_at: row.get(5)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }

    pub fn uptime_incidents(
        &self,
        limit: u32,
    ) -> Result<Vec<UptimeIncidentRecord>, AlertStoreError> {
        let limit = i64::from(limit.clamp(1, 100));
        let mut statement = self.connection.prepare(
            "SELECT incidents.id, incidents.target_id, targets.name, incidents.state,
                    incidents.started_at, incidents.recovered_at, incidents.last_error_code
             FROM uptime_incidents AS incidents
             JOIN uptime_targets AS targets ON targets.id = incidents.target_id
             ORDER BY incidents.started_at DESC, incidents.id DESC LIMIT ?1",
        )?;
        let rows = statement.query_map(params![limit], |row| {
            Ok(UptimeIncidentRecord {
                id: row.get(0)?,
                target_id: row.get(1)?,
                target_name: row.get(2)?,
                state: row.get(3)?,
                started_at: row.get(4)?,
                recovered_at: row.get(5)?,
                last_error_code: row.get(6)?,
            })
        })?;
        rows.collect::<Result<Vec<_>, _>>().map_err(Into::into)
    }
}

fn uptime_target_from_row(row: &rusqlite::Row<'_>) -> Result<UptimeTargetRecord, rusqlite::Error> {
    Ok(UptimeTargetRecord {
        id: row.get(0)?,
        name: row.get(1)?,
        url: row.get(2)?,
        expected_status: u16::try_from(row.get::<_, i64>(3)?).unwrap_or(0),
        interval_seconds: u64::try_from(row.get::<_, i64>(4)?).unwrap_or(0),
        timeout_millis: u64::try_from(row.get::<_, i64>(5)?).unwrap_or(0),
        failure_threshold: u32::try_from(row.get::<_, i64>(6)?).unwrap_or(0),
        enabled: row.get(7)?,
        status: row.get(8)?,
        consecutive_failures: u32::try_from(row.get::<_, i64>(9)?).unwrap_or(0),
        last_checked_at: row.get(10)?,
        last_http_status: row
            .get::<_, Option<i64>>(11)?
            .and_then(|value| u16::try_from(value).ok()),
        last_latency_ms: row
            .get::<_, Option<i64>>(12)?
            .and_then(|value| u64::try_from(value).ok()),
        last_error_code: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
    })
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

    #[test]
    fn transition_outbox_survives_reopen_without_duplicate_events() {
        let path =
            std::env::temp_dir().join(format!("fusionpanel-alert-test-{}.sqlite", Uuid::new_v4()));
        let key = CryptoEngine::generate_key();
        let rule_id;
        {
            let mut store = AlertStore::new(&path, Some(&key)).unwrap();
            rule_id = store
                .create_rule(&sample_rule(), "secret", "2026-11-19T10:00:00Z")
                .unwrap()
                .id;
            let result = store
                .transition_and_enqueue(
                    &rule_id,
                    "node-1",
                    true,
                    90.0,
                    "2026-11-19T10:01:00Z",
                    "{\"event\":\"triggered\"}",
                )
                .unwrap();
            assert!(result.is_some());
        }
        let mut store = AlertStore::new(&path, Some(&key)).unwrap();
        assert_eq!(store.pending_deliveries().unwrap().len(), 1);
        assert!(store
            .transition_and_enqueue(&rule_id, "node-1", true, 90.0, "2026-11-19T10:02:00Z", "{}")
            .unwrap()
            .is_none());
        assert_eq!(store.pending_deliveries().unwrap().len(), 1);
        drop(store);
        std::fs::remove_file(path).unwrap();
    }

    fn sample_uptime_target() -> NewUptimeTarget {
        NewUptimeTarget {
            name: "Local service".to_string(),
            url: "http://127.0.0.1:8080/health".to_string(),
            expected_status: 200,
            interval_seconds: 30,
            timeout_millis: 1_000,
            failure_threshold: 2,
        }
    }

    #[test]
    fn uptime_incident_requires_threshold_and_recovers_after_success() {
        let mut store = AlertStore::in_memory(None).unwrap();
        let target = store
            .create_uptime_target(&sample_uptime_target(), "2026-11-19T10:00:00Z", 1_000)
            .unwrap();

        assert_eq!(
            store
                .record_uptime_check(
                    &target.id,
                    false,
                    None,
                    1_000,
                    Some("timeout"),
                    "2026-11-19T10:00:01Z",
                    2_000,
                )
                .unwrap(),
            None
        );
        assert_eq!(store.uptime_incidents(10).unwrap().len(), 0);
        assert_eq!(
            store
                .record_uptime_check(
                    &target.id,
                    true,
                    Some(200),
                    12,
                    None,
                    "2026-11-19T10:00:10Z",
                    11_000,
                )
                .unwrap(),
            None
        );
        assert_eq!(store.uptime_incidents(10).unwrap().len(), 0);
        assert_eq!(
            store
                .record_uptime_check(
                    &target.id,
                    false,
                    Some(503),
                    40,
                    Some("unexpected_status"),
                    "2026-11-19T10:00:20Z",
                    21_000,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .record_uptime_check(
                    &target.id,
                    false,
                    Some(503),
                    40,
                    Some("unexpected_status"),
                    "2026-11-19T10:00:31Z",
                    32_000,
                )
                .unwrap(),
            Some(UptimeTransition::Down)
        );
        assert_eq!(
            store
                .record_uptime_check(
                    &target.id,
                    false,
                    Some(503),
                    35,
                    Some("unexpected_status"),
                    "2026-11-19T10:01:01Z",
                    62_000,
                )
                .unwrap(),
            None
        );
        assert_eq!(
            store
                .uptime_incidents(10)
                .unwrap()
                .iter()
                .filter(|incident| incident.state == "down")
                .count(),
            1
        );
        assert_eq!(
            store
                .record_uptime_check(
                    &target.id,
                    true,
                    Some(200),
                    12,
                    None,
                    "2026-11-19T10:01:31Z",
                    92_000,
                )
                .unwrap(),
            Some(UptimeTransition::Recovered)
        );
        let incidents = store.uptime_incidents(10).unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].state, "recovered");
        assert_eq!(
            incidents[0].recovered_at.as_deref(),
            Some("2026-11-19T10:01:31Z")
        );
        assert_eq!(store.uptime_checks(&target.id, 10).unwrap().len(), 6);
    }

    #[test]
    fn uptime_target_validation_and_disable_preserve_a_cancelled_incident() {
        let mut store = AlertStore::in_memory(None).unwrap();
        let mut invalid = sample_uptime_target();
        invalid.interval_seconds = 1;
        assert!(matches!(
            store.create_uptime_target(&invalid, "2026-11-19T10:00:00Z", 1_000),
            Err(AlertStoreError::InvalidUptimeTarget)
        ));

        let target = store
            .create_uptime_target(&sample_uptime_target(), "2026-11-19T10:00:00Z", 1_000)
            .unwrap();
        for (second, millis) in [(1, 2_000), (31, 32_000)] {
            let timestamp = format!("2026-11-19T10:00:{second:02}Z");
            let _ = store
                .record_uptime_check(
                    &target.id,
                    false,
                    None,
                    0,
                    Some("transport_error"),
                    &timestamp,
                    millis,
                )
                .unwrap();
        }
        assert_eq!(store.pending_uptime_events().unwrap().len(), 1);
        assert!(store
            .disable_uptime_target(&target.id, "2026-11-19T10:01:00Z")
            .unwrap());
        assert!(!store
            .disable_uptime_target("missing", "2026-11-19T10:01:00Z")
            .unwrap());
        assert!(store.pending_uptime_events().unwrap().is_empty());
        assert!(store
            .transition_and_enqueue(
                "missing-rule",
                &format!("uptime:{}", target.id),
                true,
                0.0,
                "now",
                "{}"
            )
            .unwrap()
            .is_none());
        let disabled = store.uptime_targets().unwrap().remove(0);
        assert!(!disabled.enabled);
        assert_eq!(disabled.status, "disabled");
        let incidents = store.uptime_incidents(10).unwrap();
        assert_eq!(incidents.len(), 1);
        assert_eq!(incidents[0].state, "cancelled");
        assert_eq!(
            incidents[0].last_error_code.as_deref(),
            Some("target_disabled")
        );
    }
}

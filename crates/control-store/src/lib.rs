#![forbid(unsafe_code)]

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

mod backups;
mod cron;
mod databases;
mod dns;
mod mail;
mod security;
mod tls;
mod websites;

pub use backups::{BackupOperation, BackupOperationReservation, BackupRecord, BackupState};
pub use cron::{CronJobRecord, CronJobState, CronOperation, CronOperationReservation};
pub use mail::{
    MailAliasRecord, MailDkimRecord, MailDomainDetails, MailDomainRecord, MailOperation,
    MailOperationReservation, MailResourceState, MailboxRecord,
};
pub use security::{ApiTokenRecord, AuditLogEntry, SecurityRuleRecord, UserMfaRecord};

pub use databases::{
    DatabaseEngine, DatabaseOperation, DatabaseOperationReservation, DatabaseRecord, DatabaseState,
    DatabaseUserRecord, DatabaseUserState,
};
pub use dns::{
    DnsOperation, DnsOperationReservation, DnsRecordEntry, DnsRecordMutation, DnsZoneRecord,
    DnsZoneState, DnsZoneWithRecords,
};
pub use tls::{
    WebsiteTlsMode, WebsiteTlsOperation, WebsiteTlsOperationReservation, WebsiteTlsRecord,
    WebsiteTlsState,
};
pub use websites::{
    WebsiteOperation, WebsiteOperationReservation, WebsiteRecord, WebsiteSpec, WebsiteState,
};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum OwnershipMode {
    TenantOwned,
    PlatformManaged,
    ExternallyManaged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ResourceRecord {
    pub tenant_id: Uuid,
    pub resource_id: Uuid,
    pub ownership_mode: OwnershipMode,
    pub revision: u64,
    pub desired_state: serde_json::Value,
    pub observed_state: Option<serde_json::Value>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeRecord {
    pub node_id: Uuid,
    pub name: String,
    pub created_at: String,
    pub last_heartbeat: String,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("Conflict: expected revision {expected}, but found {actual}")]
    Conflict { expected: u64, actual: u64 },
    #[error("Resource not found: {0}")]
    NotFound(Uuid),
    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Parse error: {0}")]
    Parse(String),
    #[error("Website not found: {0}")]
    WebsiteNotFound(Uuid),
    #[error("Website request conflicts with existing state")]
    WebsiteConflict,
    #[error("Website operation is still in progress")]
    WebsiteOperationInProgress,
    #[error("Idempotency key was reused for a different operation")]
    IdempotencyConflict,
    #[error("Database request conflicts with existing state: {0}")]
    DatabaseConflict(String),
    #[error("Database operation is still in progress")]
    DatabaseOperationInProgress,
    #[error("Mail request conflicts with existing state: {0}")]
    MailConflict(String),
    #[error("Mail operation is still in progress")]
    MailOperationInProgress,
}

pub struct ControlStore {
    conn: Connection,
}

impl ControlStore {
    pub fn new<P: AsRef<Path>>(path: P) -> Result<Self, StoreError> {
        let mut conn = Connection::open(path)?;
        conn.pragma_update(None, "journal_mode", "WAL")?;
        conn.pragma_update(None, "synchronous", "NORMAL")?;
        conn.busy_timeout(std::time::Duration::from_secs(5))?;
        Self::run_migrations(&mut conn)?;
        Ok(Self { conn })
    }

    pub fn new_in_memory() -> Result<Self, StoreError> {
        let mut conn = Connection::open_in_memory()?;
        Self::run_migrations(&mut conn)?;
        Ok(Self { conn })
    }

    fn run_migrations(conn: &mut Connection) -> Result<(), StoreError> {
        let tx = conn.transaction()?;
        let user_version: u32 = tx.pragma_query_value(None, "user_version", |row| row.get(0))?;

        if user_version < 1 {
            tx.execute(
                r#"
                CREATE TABLE resources (
                    resource_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    ownership_mode TEXT NOT NULL,
                    revision INTEGER NOT NULL,
                    desired_state TEXT NOT NULL,
                    observed_state TEXT
                );
                "#,
                [],
            )?;
            tx.execute(
                "CREATE INDEX idx_resources_tenant_id ON resources (tenant_id);",
                [],
            )?;
            tx.pragma_update(None, "user_version", 1)?;
        }

        if user_version < 2 {
            tx.execute(
                r#"
                CREATE TABLE nodes (
                    node_id TEXT PRIMARY KEY,
                    name TEXT NOT NULL,
                    token_hash TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    last_heartbeat TEXT NOT NULL
                );
                CREATE INDEX idx_nodes_name ON nodes (name);
                "#,
                [],
            )?;
            tx.pragma_update(None, "user_version", 2)?;
        }

        if user_version < 3 {
            tx.execute_batch(
                r#"
                CREATE TABLE websites (
                    site_id TEXT PRIMARY KEY,
                    domain TEXT NOT NULL COLLATE NOCASE UNIQUE,
                    spec TEXT NOT NULL,
                    state TEXT NOT NULL CHECK (state IN ('provisioning', 'active', 'failed', 'deleting')),
                    last_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    operation_request_id TEXT NOT NULL,
                    job_id TEXT,
                    tenant_id TEXT NOT NULL
                );
                CREATE INDEX idx_websites_state ON websites (state);
                CREATE INDEX idx_websites_tenant_id ON websites (tenant_id);
                CREATE TABLE website_operations (
                    idempotency_key TEXT PRIMARY KEY,
                    fingerprint TEXT NOT NULL,
                    operation TEXT NOT NULL CHECK (operation IN ('site.apply', 'site.delete')),
                    site_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    site_snapshot TEXT NOT NULL,
                    job_id TEXT,
                    previous_site_snapshot TEXT
                );
                CREATE INDEX idx_website_operations_site_id ON website_operations (site_id);
                "#,
            )?;
            tx.pragma_update(None, "user_version", 3)?;
        }

        if user_version < 4 {
            let has_tenant_id = {
                let mut statement = tx.prepare("PRAGMA table_info(websites)")?;
                let rows = statement.query_map([], |row| row.get::<_, String>(1))?;
                rows.collect::<Result<Vec<_>, _>>()?
                    .iter()
                    .any(|name| name == "tenant_id")
            };
            if !has_tenant_id {
                let website_count: i64 =
                    tx.query_row("SELECT COUNT(*) FROM websites", [], |row| row.get(0))?;
                let operation_count: i64 =
                    tx.query_row("SELECT COUNT(*) FROM website_operations", [], |row| {
                        row.get(0)
                    })?;
                if website_count > 0 || operation_count > 0 {
                    return Err(StoreError::Parse(
                        "website data exists without tenant ownership; migration stopped without changing it"
                            .to_string(),
                    ));
                }
                tx.execute_batch(
                    "ALTER TABLE websites ADD COLUMN tenant_id TEXT NOT NULL DEFAULT '';",
                )?;
            }
            tx.execute_batch(
                "CREATE INDEX IF NOT EXISTS idx_websites_tenant_id ON websites (tenant_id);",
            )?;
            tx.pragma_update(None, "user_version", 4)?;
        }

        if user_version < 5 {
            tx.execute_batch(
                r#"
                CREATE TABLE website_tls (
                    site_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    mode TEXT NOT NULL CHECK (mode IN ('custom_pem', 'acme_http01')),
                    state TEXT NOT NULL CHECK (state IN ('disabled', 'provisioning', 'active', 'failed', 'deleting')),
                    acme_directory_url TEXT,
                    contact_email TEXT,
                    last_error TEXT,
                    last_operation TEXT,
                    last_request_id TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_website_tls_tenant_id ON website_tls (tenant_id);
                CREATE TABLE website_tls_operations (
                    idempotency_key TEXT PRIMARY KEY,
                    fingerprint TEXT NOT NULL,
                    operation TEXT NOT NULL CHECK (operation IN ('tls.apply', 'tls.delete')),
                    site_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    tls_snapshot TEXT NOT NULL,
                    job_id TEXT,
                    previous_tls_snapshot TEXT
                );
                CREATE INDEX idx_website_tls_operations_site_id ON website_tls_operations (site_id);
                "#,
            )?;
            tx.pragma_update(None, "user_version", 5)?;
        }

        if user_version < 6 {
            tx.execute_batch(
                r#"
                CREATE TABLE databases (
                    database_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    engine TEXT NOT NULL CHECK (engine IN ('postgresql', 'mariadb')),
                    state TEXT NOT NULL CHECK (state IN ('creating', 'ready', 'failed', 'deleting')),
                    last_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    operation_request_id TEXT NOT NULL
                );
                CREATE UNIQUE INDEX idx_databases_name_engine ON databases (LOWER(name), engine);
                CREATE INDEX idx_databases_tenant_id ON databases (tenant_id);
                CREATE TABLE database_users (
                    user_id TEXT PRIMARY KEY,
                    database_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    username TEXT NOT NULL,
                    state TEXT NOT NULL CHECK (state IN ('creating', 'ready', 'failed')),
                    last_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    operation_request_id TEXT NOT NULL
                );
                CREATE UNIQUE INDEX idx_database_users_db_username ON database_users (database_id, LOWER(username));
                CREATE TABLE database_operations (
                    idempotency_key TEXT PRIMARY KEY,
                    fingerprint TEXT NOT NULL,
                    operation TEXT NOT NULL CHECK (operation IN ('database.apply', 'database.delete', 'database.user.create')),
                    database_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    snapshot TEXT NOT NULL,
                    user_snapshot TEXT,
                    job_id TEXT
                );
                "#,
            )?;
            tx.pragma_update(None, "user_version", 6)?;
        }

        if user_version < 7 {
            tx.execute_batch(
                r#"
                CREATE TABLE backups (
                    backup_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    archive_key TEXT NOT NULL,
                    target_json TEXT NOT NULL,
                    destination_kind TEXT NOT NULL CHECK (destination_kind IN ('local', 's3')),
                    destination_summary TEXT NOT NULL,
                    state TEXT NOT NULL CHECK (state IN ('creating', 'ready', 'restoring', 'failed')),
                    manifest_sha256 TEXT,
                    last_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    operation_request_id TEXT NOT NULL
                );
                CREATE INDEX idx_backups_tenant_id ON backups (tenant_id);
                CREATE TABLE backup_operations (
                    idempotency_key TEXT PRIMARY KEY,
                    fingerprint TEXT NOT NULL,
                    operation TEXT NOT NULL CHECK (operation IN ('backup.create', 'backup.restore')),
                    backup_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    snapshot TEXT NOT NULL,
                    job_id TEXT
                );
                "#,
            )?;
            tx.pragma_update(None, "user_version", 7)?;
        }

        if user_version < 8 {
            tx.execute_batch(
                r#"
                CREATE TABLE dns_zones (
                    zone_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    domain TEXT NOT NULL COLLATE NOCASE UNIQUE,
                    serial INTEGER NOT NULL,
                    default_ttl INTEGER NOT NULL,
                    state TEXT NOT NULL CHECK (state IN ('provisioning', 'ready', 'failed', 'deleting')),
                    last_error TEXT,
                    last_probe_result TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    operation_request_id TEXT NOT NULL
                );
                CREATE INDEX idx_dns_zones_tenant_id ON dns_zones (tenant_id);
                CREATE TABLE dns_records (
                    record_id TEXT PRIMARY KEY,
                    zone_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    record_type TEXT NOT NULL CHECK (record_type IN ('A', 'AAAA', 'CNAME', 'MX', 'TXT')),
                    value TEXT NOT NULL,
                    ttl INTEGER NOT NULL,
                    priority INTEGER,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_dns_records_zone_id ON dns_records (zone_id);
                CREATE TABLE dns_operations (
                    idempotency_key TEXT PRIMARY KEY,
                    fingerprint TEXT NOT NULL,
                    operation TEXT NOT NULL CHECK (operation IN ('dns.zone.apply', 'dns.zone.delete', 'dns.probe')),
                    zone_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    snapshot TEXT NOT NULL,
                    job_id TEXT
                );
                "#,
            )?;
            tx.pragma_update(None, "user_version", 8)?;
        }

        if user_version < 9 {
            tx.execute_batch(
                r#"
                CREATE TABLE cron_jobs (
                    cron_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    schedule TEXT NOT NULL,
                    command TEXT NOT NULL,
                    enabled INTEGER NOT NULL,
                    state TEXT NOT NULL CHECK (state IN ('provisioning', 'ready', 'failed', 'deleting')),
                    last_run_at TEXT,
                    last_run_status TEXT,
                    last_error TEXT,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    operation_request_id TEXT NOT NULL
                );
                CREATE INDEX idx_cron_jobs_tenant_id ON cron_jobs (tenant_id);
                CREATE TABLE cron_operations (
                    idempotency_key TEXT PRIMARY KEY,
                    fingerprint TEXT NOT NULL,
                    operation TEXT NOT NULL CHECK (operation IN ('cron.apply', 'cron.delete', 'cron.run_now')),
                    cron_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    snapshot TEXT NOT NULL,
                    job_id TEXT
                );
                "#,
            )?;
            tx.pragma_update(None, "user_version", 9)?;
        }

        if user_version < 10 {
            tx.execute_batch(
                r#"
                CREATE TABLE api_tokens (
                    token_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    user_id TEXT NOT NULL,
                    name TEXT NOT NULL,
                    token_hash TEXT NOT NULL UNIQUE,
                    scopes_json TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    revoked_at TEXT
                );
                CREATE INDEX idx_api_tokens_tenant_id ON api_tokens (tenant_id);
                CREATE TABLE audit_logs (
                    id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    actor_id TEXT NOT NULL,
                    action TEXT NOT NULL,
                    resource_type TEXT NOT NULL,
                    resource_id TEXT NOT NULL,
                    outcome TEXT NOT NULL,
                    detail TEXT,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX idx_audit_logs_tenant_id ON audit_logs (tenant_id);
                CREATE TABLE user_mfa (
                    user_id TEXT PRIMARY KEY,
                    totp_secret TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 0,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE TABLE security_rules (
                    rule_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    rule_type TEXT NOT NULL,
                    target TEXT NOT NULL,
                    action TEXT NOT NULL,
                    description TEXT NOT NULL,
                    created_at TEXT NOT NULL
                );
                CREATE INDEX idx_security_rules_tenant_id ON security_rules (tenant_id);
                "#,
            )?;
            tx.pragma_update(None, "user_version", 10)?;
        }

        if user_version < 11 {
            let has_nodes_table: bool = tx.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'nodes')",
                [],
                |row| row.get(0),
            )?;
            if has_nodes_table {
                let has_nodes_tenant_id: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('nodes') WHERE name = 'tenant_id')",
                    [],
                    |row| row.get(0),
                )?;
                if !has_nodes_tenant_id {
                    tx.execute("ALTER TABLE nodes ADD COLUMN tenant_id TEXT", [])?;
                }
            }
            tx.execute_batch(
                r#"
                CREATE TABLE mail_domains (
                    domain_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    node_id TEXT NOT NULL,
                    domain TEXT NOT NULL UNIQUE,
                    dns_zone_id TEXT,
                    active INTEGER NOT NULL DEFAULT 1,
                    state TEXT NOT NULL CHECK (state IN ('provisioning', 'ready', 'failed', 'deleting')),
                    desired_revision INTEGER NOT NULL DEFAULT 1,
                    queued_revision INTEGER,
                    applied_revision INTEGER,
                    failed_revision INTEGER,
                    pending_job_id TEXT,
                    last_error TEXT,
                    operation_request_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_mail_domains_tenant_id ON mail_domains (tenant_id);
                CREATE INDEX idx_mail_domains_node_id ON mail_domains (node_id);

                CREATE TABLE mail_mailboxes (
                    mailbox_id TEXT PRIMARY KEY,
                    domain_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    address TEXT NOT NULL UNIQUE,
                    local_part TEXT NOT NULL,
                    quota_bytes INTEGER NOT NULL,
                    active INTEGER NOT NULL DEFAULT 1,
                    password_hash TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_mail_mailboxes_domain_id ON mail_mailboxes (domain_id);
                CREATE INDEX idx_mail_mailboxes_tenant_id ON mail_mailboxes (tenant_id);

                CREATE TABLE mail_aliases (
                    alias_id TEXT PRIMARY KEY,
                    domain_id TEXT NOT NULL,
                    tenant_id TEXT NOT NULL,
                    source TEXT NOT NULL UNIQUE,
                    destinations_json TEXT NOT NULL,
                    active INTEGER NOT NULL DEFAULT 1,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_mail_aliases_domain_id ON mail_aliases (domain_id);
                CREATE INDEX idx_mail_aliases_tenant_id ON mail_aliases (tenant_id);

                CREATE TABLE mail_dkim_keys (
                    domain_id TEXT PRIMARY KEY,
                    tenant_id TEXT NOT NULL,
                    selector TEXT NOT NULL,
                    enabled INTEGER NOT NULL DEFAULT 1,
                    dns_zone_id TEXT,
                    public_key_pem TEXT,
                    txt_record TEXT,
                    state TEXT NOT NULL CHECK (state IN ('provisioning', 'ready', 'failed', 'deleting')),
                    queued_revision INTEGER,
                    applied_revision INTEGER,
                    failed_revision INTEGER,
                    pending_job_id TEXT,
                    last_error TEXT,
                    operation_request_id TEXT NOT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL
                );
                CREATE INDEX idx_mail_dkim_keys_tenant_id ON mail_dkim_keys (tenant_id);

                CREATE TABLE mail_operations (
                    idempotency_key TEXT PRIMARY KEY,
                    fingerprint TEXT NOT NULL,
                    operation TEXT NOT NULL CHECK (operation IN ('mail.domain.apply', 'mail.domain.delete', 'mail.dkim.apply')),
                    domain_id TEXT NOT NULL,
                    request_id TEXT NOT NULL,
                    revision INTEGER NOT NULL,
                    snapshot TEXT NOT NULL,
                    job_id TEXT
                );
                CREATE INDEX idx_mail_operations_request_id ON mail_operations (request_id);
                "#,
            )?;
            tx.pragma_update(None, "user_version", 11)?;
        }

        tx.commit()?;
        Ok(())
    }

    pub fn register_node(
        &mut self,
        proposed_node_id: Uuid,
        name: &str,
        token_hash: &str,
        now: &str,
    ) -> Result<Uuid, StoreError> {
        // Serialize lookup and insert/update so concurrent API instances using
        // the same SQLite database cannot create duplicate rows for one name.
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing_id = tx
            .query_row(
                "SELECT node_id FROM nodes WHERE name = ?1 ORDER BY created_at, node_id LIMIT 1",
                [name],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        if let Some(existing_id) = existing_id {
            let existing_id = Uuid::parse_str(&existing_id)
                .map_err(|error| StoreError::Parse(error.to_string()))?;
            tx.execute(
                "UPDATE nodes SET token_hash = ?1, last_heartbeat = ?2 WHERE node_id = ?3",
                params![token_hash, now, existing_id.to_string()],
            )?;
            tx.commit()?;
            return Ok(existing_id);
        }

        tx.execute(
            "INSERT INTO nodes (node_id, name, token_hash, created_at, last_heartbeat) VALUES (?1, ?2, ?3, ?4, ?4)",
            params![proposed_node_id.to_string(), name, token_hash, now],
        )?;
        tx.commit()?;
        Ok(proposed_node_id)
    }

    pub fn list_nodes(&self) -> Result<Vec<NodeRecord>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT node_id, name, created_at, last_heartbeat FROM nodes ORDER BY created_at, node_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;

        let mut nodes = Vec::new();
        for row in rows {
            let (id, name, created_at, last_heartbeat) = row?;
            nodes.push(NodeRecord {
                node_id: Uuid::parse_str(&id)
                    .map_err(|error| StoreError::Parse(error.to_string()))?,
                name,
                created_at,
                last_heartbeat,
            });
        }
        Ok(nodes)
    }

    pub fn node_token_hash(&self, node_id: Uuid) -> Result<Option<String>, StoreError> {
        Ok(self
            .conn
            .query_row(
                "SELECT token_hash FROM nodes WHERE node_id = ?1",
                [node_id.to_string()],
                |row| row.get(0),
            )
            .optional()?)
    }

    pub fn touch_node(&mut self, node_id: Uuid, now: &str) -> Result<bool, StoreError> {
        let changed = self.conn.execute(
            "UPDATE nodes SET last_heartbeat = ?1 WHERE node_id = ?2",
            params![now, node_id.to_string()],
        )?;
        Ok(changed == 1)
    }

    /// Returns `None` if the node does not exist, `Some(None)` if global/shared,
    /// or `Some(Some(tenant_id))` if bound to a specific tenant.
    pub fn get_node_tenant(&self, node_id: Uuid) -> Result<Option<Option<Uuid>>, StoreError> {
        let row: Option<Option<String>> = self
            .conn
            .query_row(
                "SELECT tenant_id FROM nodes WHERE node_id = ?1",
                [node_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        match row {
            None => Ok(None),
            Some(None) => Ok(Some(None)),
            Some(Some(tid)) => {
                let parsed =
                    Uuid::parse_str(&tid).map_err(|error| StoreError::Parse(error.to_string()))?;
                Ok(Some(Some(parsed)))
            }
        }
    }

    pub fn set_node_tenant(
        &mut self,
        node_id: Uuid,
        tenant_id: Option<Uuid>,
    ) -> Result<bool, StoreError> {
        let changed = self.conn.execute(
            "UPDATE nodes SET tenant_id = ?1 WHERE node_id = ?2",
            params![tenant_id.map(|t| t.to_string()), node_id.to_string()],
        )?;
        Ok(changed == 1)
    }

    pub fn create_resource(
        &mut self,
        tenant_id: Uuid,
        resource_id: Uuid,
        ownership_mode: OwnershipMode,
        desired_state: serde_json::Value,
    ) -> Result<(), StoreError> {
        let mode_str = serde_json::to_string(&ownership_mode)?;
        let desired_str = serde_json::to_string(&desired_state)?;

        self.conn.execute(
            r#"
            INSERT INTO resources (resource_id, tenant_id, ownership_mode, revision, desired_state, observed_state)
            VALUES (?1, ?2, ?3, 1, ?4, NULL)
            "#,
            params![
                resource_id.to_string(),
                tenant_id.to_string(),
                mode_str,
                desired_str,
            ],
        )?;

        Ok(())
    }

    pub fn get_resource(&self, resource_id: Uuid) -> Result<ResourceRecord, StoreError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT tenant_id, ownership_mode, revision, desired_state, observed_state
            FROM resources
            WHERE resource_id = ?1
            "#,
        )?;

        let row = stmt
            .query_row(params![resource_id.to_string()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, u64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            })
            .optional()?
            .ok_or(StoreError::NotFound(resource_id))?;

        let tenant_id = Uuid::parse_str(&row.0).map_err(|e| StoreError::Parse(e.to_string()))?;
        let ownership_mode = serde_json::from_str(&row.1)?;
        let desired_state = serde_json::from_str(&row.3)?;
        let observed_state = row.4.map(|s| serde_json::from_str(&s)).transpose()?;

        Ok(ResourceRecord {
            tenant_id,
            resource_id,
            ownership_mode,
            revision: row.2,
            desired_state,
            observed_state,
        })
    }

    pub fn tenant_exists(&self, tenant_id: Uuid) -> Result<bool, StoreError> {
        self.conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM resources WHERE tenant_id = ?1)",
                [tenant_id.to_string()],
                |row| row.get(0),
            )
            .map_err(StoreError::from)
    }

    pub fn list_tenant_resources(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<ResourceRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT resource_id, ownership_mode, revision, desired_state, observed_state
            FROM resources
            WHERE tenant_id = ?1
            "#,
        )?;

        let rows = stmt.query_map(params![tenant_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, u64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, Option<String>>(4)?,
            ))
        })?;

        let mut resources = Vec::new();
        for row_result in rows {
            let row = row_result?;
            let resource_id =
                Uuid::parse_str(&row.0).map_err(|e| StoreError::Parse(e.to_string()))?;
            let ownership_mode = serde_json::from_str(&row.1)?;
            let desired_state = serde_json::from_str(&row.3)?;
            let observed_state = row.4.map(|s| serde_json::from_str(&s)).transpose()?;

            resources.push(ResourceRecord {
                tenant_id,
                resource_id,
                ownership_mode,
                revision: row.2,
                desired_state,
                observed_state,
            });
        }

        Ok(resources)
    }

    pub fn update_desired_state(
        &mut self,
        resource_id: Uuid,
        expected_revision: u64,
        new_state: serde_json::Value,
    ) -> Result<u64, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let current_revision: u64 = tx
            .query_row(
                "SELECT revision FROM resources WHERE resource_id = ?1",
                params![resource_id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound(resource_id))?;

        if current_revision != expected_revision {
            return Err(StoreError::Conflict {
                expected: expected_revision,
                actual: current_revision,
            });
        }

        let new_revision = current_revision + 1;
        let new_state_str = serde_json::to_string(&new_state)?;

        tx.execute(
            r#"
            UPDATE resources
            SET revision = ?1, desired_state = ?2
            WHERE resource_id = ?3
            "#,
            params![new_revision, new_state_str, resource_id.to_string()],
        )?;

        tx.commit()?;
        Ok(new_revision)
    }

    pub fn update_observed_state(
        &mut self,
        resource_id: Uuid,
        expected_revision: u64,
        new_observed_state: serde_json::Value,
    ) -> Result<u64, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let current_revision: u64 = tx
            .query_row(
                "SELECT revision FROM resources WHERE resource_id = ?1",
                params![resource_id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound(resource_id))?;

        if current_revision != expected_revision {
            return Err(StoreError::Conflict {
                expected: expected_revision,
                actual: current_revision,
            });
        }

        let new_revision = current_revision + 1;
        let new_state_str = serde_json::to_string(&new_observed_state)?;

        tx.execute(
            r#"
            UPDATE resources
            SET revision = ?1, observed_state = ?2
            WHERE resource_id = ?3
            "#,
            params![new_revision, new_state_str, resource_id.to_string()],
        )?;

        tx.commit()?;
        Ok(new_revision)
    }

    pub fn get_tenant_resource(
        &self,
        tenant_id: Uuid,
        resource_id: Uuid,
    ) -> Result<ResourceRecord, StoreError> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT tenant_id, ownership_mode, revision, desired_state, observed_state
            FROM resources
            WHERE resource_id = ?1 AND tenant_id = ?2
            "#,
        )?;

        let row = stmt
            .query_row(
                params![resource_id.to_string(), tenant_id.to_string()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, u64>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StoreError::NotFound(resource_id))?;

        let tenant_id = Uuid::parse_str(&row.0).map_err(|e| StoreError::Parse(e.to_string()))?;
        let ownership_mode = serde_json::from_str(&row.1)?;
        let desired_state = serde_json::from_str(&row.3)?;
        let observed_state = row.4.map(|s| serde_json::from_str(&s)).transpose()?;

        Ok(ResourceRecord {
            tenant_id,
            resource_id,
            ownership_mode,
            revision: row.2,
            desired_state,
            observed_state,
        })
    }

    pub fn update_tenant_desired_state(
        &mut self,
        tenant_id: Uuid,
        resource_id: Uuid,
        expected_revision: u64,
        new_state: serde_json::Value,
    ) -> Result<u64, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        let current_revision: u64 = tx
            .query_row(
                "SELECT revision FROM resources WHERE resource_id = ?1 AND tenant_id = ?2",
                params![resource_id.to_string(), tenant_id.to_string()],
                |row| row.get(0),
            )
            .optional()?
            .ok_or(StoreError::NotFound(resource_id))?;

        if current_revision != expected_revision {
            return Err(StoreError::Conflict {
                expected: expected_revision,
                actual: current_revision,
            });
        }

        let new_revision = current_revision + 1;
        let new_state_str = serde_json::to_string(&new_state)?;

        tx.execute(
            r#"
            UPDATE resources
            SET revision = ?1, desired_state = ?2
            WHERE resource_id = ?3 AND tenant_id = ?4
            "#,
            params![
                new_revision,
                new_state_str,
                resource_id.to_string(),
                tenant_id.to_string()
            ],
        )?;

        tx.commit()?;
        Ok(new_revision)
    }

    pub fn delete_tenant_resource(
        &mut self,
        tenant_id: Uuid,
        resource_id: Uuid,
    ) -> Result<(), StoreError> {
        let rows = self.conn.execute(
            "DELETE FROM resources WHERE resource_id = ?1 AND tenant_id = ?2",
            params![resource_id.to_string(), tenant_id.to_string()],
        )?;
        if rows == 0 {
            Err(StoreError::NotFound(resource_id))
        } else {
            Ok(())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::thread;
    use std::time::SystemTime;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let timestamp = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let count = TEMP_COUNTER.fetch_add(1, Ordering::SeqCst);
            let target_dir = std::env::current_dir().unwrap().join("target");
            let _ = fs::create_dir_all(&target_dir);
            let path = target_dir.join(format!("test_dir_{}_{}", timestamp, count));
            fs::create_dir(&path).unwrap();
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_mode(0o700);
            fs::set_permissions(&path, perms).unwrap();
            Self { path }
        }

        fn path(&self) -> &std::path::Path {
            &self.path
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn test_store_creation_and_reopen() {
        let temp_dir = TempDir::new();
        let db_path = temp_dir.path().join("store.db");

        let tenant_id = Uuid::new_v4();
        let resource_id = Uuid::new_v4();
        let desired = json!({"status": "active"});

        {
            let mut store = ControlStore::new(&db_path).unwrap();
            store
                .create_resource(
                    tenant_id,
                    resource_id,
                    OwnershipMode::TenantOwned,
                    desired.clone(),
                )
                .unwrap();
        }

        {
            let store = ControlStore::new(&db_path).unwrap();
            let record = store.get_resource(resource_id).unwrap();
            assert_eq!(record.tenant_id, tenant_id);
            assert_eq!(record.desired_state, desired);
            assert_eq!(record.revision, 1);
        }
    }

    #[test]
    fn test_revision_conflict() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant_id = Uuid::new_v4();
        let resource_id = Uuid::new_v4();

        store
            .create_resource(
                tenant_id,
                resource_id,
                OwnershipMode::PlatformManaged,
                json!({}),
            )
            .unwrap();

        // Update with correct revision
        let new_rev = store
            .update_desired_state(resource_id, 1, json!({"status": "pending"}))
            .unwrap();
        assert_eq!(new_rev, 2);

        // Update with wrong revision
        let err = store
            .update_desired_state(resource_id, 1, json!({"status": "active"}))
            .unwrap_err();
        match err {
            StoreError::Conflict { expected, actual } => {
                assert_eq!(expected, 1);
                assert_eq!(actual, 2);
            }
            _ => panic!("Expected Conflict error"),
        }
    }

    #[test]
    fn test_ownership_filters() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant1 = Uuid::new_v4();
        let tenant2 = Uuid::new_v4();

        let r1 = Uuid::new_v4();
        let r2 = Uuid::new_v4();
        let r3 = Uuid::new_v4();

        store
            .create_resource(tenant1, r1, OwnershipMode::TenantOwned, json!({}))
            .unwrap();
        store
            .create_resource(tenant1, r2, OwnershipMode::PlatformManaged, json!({}))
            .unwrap();
        store
            .create_resource(tenant2, r3, OwnershipMode::ExternallyManaged, json!({}))
            .unwrap();

        let t1_resources = store.list_tenant_resources(tenant1).unwrap();
        assert_eq!(t1_resources.len(), 2);
        assert!(t1_resources.iter().any(|r| r.resource_id == r1));
        assert!(t1_resources.iter().any(|r| r.resource_id == r2));

        let t2_resources = store.list_tenant_resources(tenant2).unwrap();
        assert_eq!(t2_resources.len(), 1);
        assert_eq!(t2_resources[0].resource_id, r3);
    }

    #[test]
    fn test_concurrent_transactions() {
        let temp_dir = TempDir::new();
        let db_path = temp_dir.path().join("store.db");
        let tenant_id = Uuid::new_v4();
        let resource_id = Uuid::new_v4();

        {
            let mut store = ControlStore::new(&db_path).unwrap();
            store
                .create_resource(
                    tenant_id,
                    resource_id,
                    OwnershipMode::TenantOwned,
                    json!({}),
                )
                .unwrap();
        }

        let path1 = db_path.clone();
        let t1 = thread::spawn(move || {
            let mut store = ControlStore::new(&path1).unwrap();
            let mut record = store.get_resource(resource_id).unwrap();
            let mut retries = 0;
            while retries < 10 {
                match store.update_desired_state(resource_id, record.revision, json!({"a": 1})) {
                    Ok(_) => break,
                    Err(StoreError::Conflict { .. }) => {
                        record = store.get_resource(resource_id).unwrap();
                    }
                    Err(StoreError::Database(e))
                        if e.to_string().contains("database is locked") =>
                    {
                        thread::sleep(std::time::Duration::from_millis(50));
                        retries += 1;
                    }
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
        });

        let path2 = db_path.clone();
        let t2 = thread::spawn(move || {
            let mut store = ControlStore::new(&path2).unwrap();
            let mut record = store.get_resource(resource_id).unwrap();
            let mut retries = 0;
            while retries < 10 {
                match store.update_observed_state(resource_id, record.revision, json!({"b": 2})) {
                    Ok(_) => break,
                    Err(StoreError::Conflict { .. }) => {
                        record = store.get_resource(resource_id).unwrap();
                    }
                    Err(StoreError::Database(e))
                        if e.to_string().contains("database is locked") =>
                    {
                        thread::sleep(std::time::Duration::from_millis(50));
                        retries += 1;
                    }
                    Err(e) => panic!("Unexpected error: {:?}", e),
                }
            }
        });

        t1.join().unwrap();
        t2.join().unwrap();

        let store = ControlStore::new(&db_path).unwrap();
        let record = store.get_resource(resource_id).unwrap();
        assert_eq!(record.revision, 3); // 1 + 1 for each thread
        assert_eq!(record.desired_state, json!({"a": 1}));
        assert_eq!(record.observed_state, Some(json!({"b": 2})));
    }

    #[test]
    fn test_crash_safe_migrations() {
        let temp_dir = TempDir::new();
        let db_path = temp_dir.path().join("store.db");

        // First pass: apply migrations
        {
            let mut conn = Connection::open(&db_path).unwrap();
            ControlStore::run_migrations(&mut conn).unwrap();
        }

        // Second pass: should be a no-op / safe to rerun
        {
            let store = ControlStore::new(&db_path).unwrap();
            let conn = store.conn;
            let user_version: u32 = conn
                .query_row("PRAGMA user_version", [], |row| row.get(0))
                .unwrap();
            assert_eq!(user_version, 11);
            let website_tables: u32 = conn
                .query_row(
                    "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('websites', 'website_operations')",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(website_tables, 2);
            let has_tenant_id: bool = conn
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM pragma_table_info('websites') WHERE name = 'tenant_id')",
                    [],
                    |row| row.get(0),
                )
                .unwrap();
            assert!(has_tenant_id);
        }
    }

    fn create_legacy_v3_website_schema(conn: &Connection) {
        conn.execute_batch(
            r#"
            CREATE TABLE websites (
                site_id TEXT PRIMARY KEY,
                domain TEXT NOT NULL COLLATE NOCASE UNIQUE,
                spec TEXT NOT NULL,
                state TEXT NOT NULL,
                last_error TEXT,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                operation_request_id TEXT NOT NULL,
                job_id TEXT
            );
            CREATE TABLE website_operations (
                idempotency_key TEXT PRIMARY KEY,
                fingerprint TEXT NOT NULL,
                operation TEXT NOT NULL,
                site_id TEXT NOT NULL,
                request_id TEXT NOT NULL,
                site_snapshot TEXT NOT NULL,
                job_id TEXT,
                previous_site_snapshot TEXT
            );
            PRAGMA user_version = 3;
            "#,
        )
        .unwrap();
    }

    #[test]
    fn tenant_migration_adds_column_to_empty_pre_release_schema() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_legacy_v3_website_schema(&conn);

        ControlStore::run_migrations(&mut conn).unwrap();

        let user_version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_version, 11);
        let has_tenant_id: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM pragma_table_info('websites') WHERE name = 'tenant_id')",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(has_tenant_id);
    }

    #[test]
    fn tenant_migration_refuses_to_guess_ownership_of_legacy_sites() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_legacy_v3_website_schema(&conn);
        conn.execute(
            "INSERT INTO websites (site_id, domain, spec, state, created_at, updated_at, operation_request_id) VALUES (?1, ?2, ?3, ?4, ?5, ?5, ?6)",
            params![
                Uuid::new_v4().to_string(),
                "legacy.example.test",
                r#"{"kind":"static","document_root":"/srv/fusionpanel/sites/legacy.example.test"}"#,
                "active",
                "2026-11-16T00:00:00Z",
                Uuid::new_v4().to_string(),
            ],
        )
        .unwrap();

        let result = ControlStore::run_migrations(&mut conn);
        assert!(
            matches!(result, Err(StoreError::Parse(message)) if message.contains("without tenant ownership"))
        );

        let website_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM websites", [], |row| row.get(0))
            .unwrap();
        assert_eq!(website_count, 1);
        let user_version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_version, 3);
    }

    #[test]
    fn tenant_migration_refuses_to_guess_ownership_of_legacy_operation_snapshots() {
        let mut conn = Connection::open_in_memory().unwrap();
        create_legacy_v3_website_schema(&conn);
        conn.execute(
            "INSERT INTO website_operations (idempotency_key, fingerprint, operation, site_id, request_id, site_snapshot) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                "legacy-create",
                "{}",
                "site.apply",
                Uuid::new_v4().to_string(),
                Uuid::new_v4().to_string(),
                r#"{"record":{"id":"legacy","domain":"legacy.example.test","kind":"static","document_root":"/srv/fusionpanel/sites/legacy.example.test","state":"active","last_error":null,"created_at":"2026-11-16T00:00:00Z","updated_at":"2026-11-16T00:00:00Z"},"request_id":"legacy","job_id":null}"#,
            ],
        )
        .unwrap();

        let result = ControlStore::run_migrations(&mut conn);
        assert!(
            matches!(result, Err(StoreError::Parse(message)) if message.contains("without tenant ownership"))
        );

        let operation_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM website_operations", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(operation_count, 1);
        let user_version: u32 = conn
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .unwrap();
        assert_eq!(user_version, 3);
    }

    #[test]
    fn test_tenant_isolation_boundary() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let resource_id = Uuid::new_v4();

        store
            .create_resource(
                tenant_a,
                resource_id,
                OwnershipMode::TenantOwned,
                json!({"config": "secret_a"}),
            )
            .unwrap();

        // Tenant A can access and update
        let res_a = store.get_tenant_resource(tenant_a, resource_id).unwrap();
        assert_eq!(res_a.desired_state, json!({"config": "secret_a"}));

        // Tenant B CANNOT read resource_id
        let err_read = store
            .get_tenant_resource(tenant_b, resource_id)
            .unwrap_err();
        assert!(matches!(err_read, StoreError::NotFound(_)));

        // Tenant B CANNOT update resource_id
        let err_update = store
            .update_tenant_desired_state(tenant_b, resource_id, 1, json!({"config": "hacked"}))
            .unwrap_err();
        assert!(matches!(err_update, StoreError::NotFound(_)));

        // Tenant B CANNOT delete resource_id
        let err_delete = store
            .delete_tenant_resource(tenant_b, resource_id)
            .unwrap_err();
        assert!(matches!(err_delete, StoreError::NotFound(_)));

        // State for Tenant A is completely untouched
        let res_a_after = store.get_tenant_resource(tenant_a, resource_id).unwrap();
        assert_eq!(res_a_after.revision, 1);
        assert_eq!(res_a_after.desired_state, json!({"config": "secret_a"}));

        // Tenant A deletes successfully
        store.delete_tenant_resource(tenant_a, resource_id).unwrap();
        assert!(store.get_tenant_resource(tenant_a, resource_id).is_err());
    }

    #[test]
    fn test_node_registration_and_heartbeat_persist_without_listing_token_hash() {
        let temp_dir = TempDir::new();
        let db_path = temp_dir.path().join("nodes.db");
        let node_id = Uuid::new_v4();
        {
            let mut store = ControlStore::new(&db_path).unwrap();
            assert_eq!(
                store
                    .register_node(node_id, "worker-1", "stored-hash", "2026-11-15T12:00:00Z")
                    .unwrap(),
                node_id
            );
            assert_eq!(
                store.node_token_hash(node_id).unwrap().as_deref(),
                Some("stored-hash")
            );
            assert!(store.touch_node(node_id, "2026-11-15T12:00:02Z").unwrap());
        }
        let store = ControlStore::new(&db_path).unwrap();
        let nodes = store.list_nodes().unwrap();
        assert_eq!(nodes.len(), 1);
        assert_eq!(nodes[0].node_id, node_id);
        assert_eq!(nodes[0].last_heartbeat, "2026-11-15T12:00:02Z");
    }

    #[test]
    fn test_node_reregistration_keeps_identity_and_rotates_token_hash() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let original_id = Uuid::new_v4();
        let proposed_id = Uuid::new_v4();

        assert_eq!(
            store
                .register_node(
                    original_id,
                    "worker-1",
                    "first-hash",
                    "2026-11-15T12:00:00Z"
                )
                .unwrap(),
            original_id
        );
        assert_eq!(
            store
                .register_node(
                    proposed_id,
                    "worker-1",
                    "second-hash",
                    "2026-11-15T12:01:00Z"
                )
                .unwrap(),
            original_id
        );
        assert_eq!(
            store.node_token_hash(original_id).unwrap().as_deref(),
            Some("second-hash")
        );
        assert_eq!(store.list_nodes().unwrap().len(), 1);
    }
}

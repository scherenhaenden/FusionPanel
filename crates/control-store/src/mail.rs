#![forbid(unsafe_code)]

use crate::{ControlStore, StoreError};
use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MailResourceState {
    Provisioning,
    Ready,
    Failed,
    Deleting,
}

impl MailResourceState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Deleting => "deleting",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "provisioning" => Ok(Self::Provisioning),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            "deleting" => Ok(Self::Deleting),
            other => Err(StoreError::Parse(format!(
                "unknown mail resource state '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailDomainRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub node_id: Uuid,
    pub domain: String,
    pub dns_zone_id: Option<Uuid>,
    pub active: bool,
    pub state: MailResourceState,
    pub desired_revision: u64,
    pub queued_revision: Option<u64>,
    pub applied_revision: Option<u64>,
    pub failed_revision: Option<u64>,
    pub pending_job_id: Option<Uuid>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailboxRecord {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub tenant_id: Uuid,
    pub address: String,
    pub local_part: String,
    pub quota_bytes: u64,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailAliasRecord {
    pub id: Uuid,
    pub domain_id: Uuid,
    pub tenant_id: Uuid,
    pub source: String,
    pub destinations: Vec<String>,
    pub active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailDkimRecord {
    pub domain_id: Uuid,
    pub tenant_id: Uuid,
    pub selector: String,
    pub enabled: bool,
    pub dns_zone_id: Option<Uuid>,
    pub txt_record: Option<String>,
    pub public_key_pem: Option<String>,
    pub state: MailResourceState,
    pub queued_revision: Option<u64>,
    pub applied_revision: Option<u64>,
    pub failed_revision: Option<u64>,
    pub pending_job_id: Option<Uuid>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MailDomainDetails {
    pub domain: MailDomainRecord,
    pub mailboxes: Vec<MailboxRecord>,
    pub aliases: Vec<MailAliasRecord>,
    pub dkim: Option<MailDkimRecord>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum MailOperation {
    DomainApply,
    DomainDelete,
    DkimApply,
}

impl MailOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DomainApply => "mail.domain.apply",
            Self::DomainDelete => "mail.domain.delete",
            Self::DkimApply => "mail.dkim.apply",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "mail.domain.apply" => Ok(Self::DomainApply),
            "mail.domain.delete" => Ok(Self::DomainDelete),
            "mail.dkim.apply" => Ok(Self::DkimApply),
            other => Err(StoreError::Parse(format!(
                "unknown mail operation '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailOperationReservation {
    pub domain: MailDomainRecord,
    pub mailboxes: Vec<MailboxRecord>,
    pub aliases: Vec<MailAliasRecord>,
    pub dkim: Option<MailDkimRecord>,
    pub target_mailbox: Option<MailboxRecord>,
    pub target_alias: Option<MailAliasRecord>,
    pub request_id: Uuid,
    pub revision: u64,
    pub job_id: Option<Uuid>,
    pub idempotent_replay: bool,
    /// Only populated for newly reserved operations when mailbox password hashes are needed for agent apply.
    #[serde(skip)]
    pub mailbox_secrets: Vec<(Uuid, String)>,
}

impl ControlStore {
    pub fn list_mail_domains(&self, tenant_id: Uuid) -> Result<Vec<MailDomainRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT domain_id, tenant_id, node_id, domain, dns_zone_id, active, state,
                    desired_revision, queued_revision, applied_revision, failed_revision,
                    pending_job_id, last_error, created_at, updated_at
             FROM mail_domains
             WHERE tenant_id = ?1
             ORDER BY domain ASC, domain_id ASC",
        )?;
        let rows = stmt.query_map([tenant_id.to_string()], row_to_mail_domain)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }

    pub fn get_mail_domain_details(
        &self,
        domain_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<MailDomainDetails>, StoreError> {
        let domain = self
            .conn
            .query_row(
                "SELECT domain_id, tenant_id, node_id, domain, dns_zone_id, active, state,
                        desired_revision, queued_revision, applied_revision, failed_revision,
                        pending_job_id, last_error, created_at, updated_at
                 FROM mail_domains
                 WHERE domain_id = ?1 AND tenant_id = ?2",
                params![domain_id.to_string(), tenant_id.to_string()],
                row_to_mail_domain,
            )
            .optional()?;
        let Some(domain) = domain else {
            return Ok(None);
        };
        let mailboxes = load_domain_mailboxes(&self.conn, domain_id)?;
        let aliases = load_domain_aliases(&self.conn, domain_id)?;
        let dkim = load_domain_dkim(&self.conn, domain_id)?;
        Ok(Some(MailDomainDetails {
            domain,
            mailboxes,
            aliases,
            dkim,
        }))
    }

    pub fn list_mail_mailboxes(
        &self,
        domain_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<Vec<MailboxRecord>>, StoreError> {
        if !domain_owned_by_tenant(&self.conn, domain_id, tenant_id)? {
            return Ok(None);
        }
        Ok(Some(load_domain_mailboxes(&self.conn, domain_id)?))
    }

    pub fn get_mail_mailbox(
        &self,
        domain_id: Uuid,
        mailbox_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<MailboxRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT mailbox_id, domain_id, tenant_id, address, local_part, quota_bytes, active, created_at, updated_at
                 FROM mail_mailboxes
                 WHERE mailbox_id = ?1 AND domain_id = ?2 AND tenant_id = ?3",
                params![
                    mailbox_id.to_string(),
                    domain_id.to_string(),
                    tenant_id.to_string()
                ],
                row_to_mailbox,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list_mail_aliases(
        &self,
        domain_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<Vec<MailAliasRecord>>, StoreError> {
        if !domain_owned_by_tenant(&self.conn, domain_id, tenant_id)? {
            return Ok(None);
        }
        Ok(Some(load_domain_aliases(&self.conn, domain_id)?))
    }

    pub fn get_mail_alias(
        &self,
        domain_id: Uuid,
        alias_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<MailAliasRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT alias_id, domain_id, tenant_id, source, destinations_json, active, created_at, updated_at
                 FROM mail_aliases
                 WHERE alias_id = ?1 AND domain_id = ?2 AND tenant_id = ?3",
                params![
                    alias_id.to_string(),
                    domain_id.to_string(),
                    tenant_id.to_string()
                ],
                row_to_alias,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn get_mail_dkim(
        &self,
        domain_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<MailDkimRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT domain_id, tenant_id, selector, enabled, dns_zone_id, txt_record, public_key_pem,
                        state, queued_revision, applied_revision, failed_revision, pending_job_id,
                        last_error, created_at, updated_at
                 FROM mail_dkim_keys
                 WHERE domain_id = ?1 AND tenant_id = ?2",
                params![domain_id.to_string(), tenant_id.to_string()],
                row_to_dkim,
            )
            .optional()
            .map_err(StoreError::from)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_mail_domain_create(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        tenant_id: Uuid,
        node_id: Uuid,
        domain: &str,
        dns_zone_id: Option<Uuid>,
        active: bool,
        now: &str,
    ) -> Result<MailOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(replay);
        }

        let exists: Option<String> = tx
            .query_row(
                "SELECT domain_id FROM mail_domains WHERE domain = ?1",
                [domain],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_some() {
            return Err(StoreError::MailConflict(format!(
                "Mail domain '{domain}' already exists"
            )));
        }

        if let Some(zone_id) = dns_zone_id {
            validate_dns_zone_ownership(&tx, zone_id, tenant_id, domain)?;
        }

        let domain_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let revision = 1u64;

        tx.execute(
            "INSERT INTO mail_domains (
                domain_id, tenant_id, node_id, domain, dns_zone_id, active, state,
                desired_revision, queued_revision, applied_revision, failed_revision,
                pending_job_id, last_error, operation_request_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'provisioning', 1, 1, NULL, NULL, NULL, NULL, ?7, ?8, ?8)",
            params![
                domain_id.to_string(),
                tenant_id.to_string(),
                node_id.to_string(),
                domain,
                dns_zone_id.map(|id| id.to_string()),
                i64::from(active),
                request_id.to_string(),
                now,
            ],
        )?;

        let domain_rec = load_domain_tx(&tx, domain_id)?;
        let reservation = MailOperationReservation {
            domain: domain_rec,
            mailboxes: Vec::new(),
            aliases: Vec::new(),
            dkim: None,
            target_mailbox: None,
            target_alias: None,
            request_id,
            revision,
            job_id: None,
            idempotent_replay: false,
            mailbox_secrets: Vec::new(),
        };
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(reservation)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_mail_domain_update(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        tenant_id: Uuid,
        node_id: Option<Uuid>,
        dns_zone_id: Option<Option<Uuid>>,
        active: Option<bool>,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing)?;

        let new_node_id = node_id.unwrap_or(existing.node_id);
        let new_dns_zone_id = match dns_zone_id {
            Some(v) => v,
            None => existing.dns_zone_id,
        };
        let new_active = active.unwrap_or(existing.active);

        if let Some(zone_id) = new_dns_zone_id {
            validate_dns_zone_ownership(&tx, zone_id, tenant_id, &existing.domain)?;
        }

        let request_id = Uuid::new_v4();
        let revision = existing.desired_revision.saturating_add(1);
        tx.execute(
            "UPDATE mail_domains
             SET node_id = ?1, dns_zone_id = ?2, active = ?3, state = 'provisioning',
                 desired_revision = ?4, queued_revision = ?4, pending_job_id = NULL,
                 last_error = NULL, operation_request_id = ?5, updated_at = ?6
             WHERE domain_id = ?7",
            params![
                new_node_id.to_string(),
                new_dns_zone_id.map(|id| id.to_string()),
                i64::from(new_active),
                revision as i64,
                request_id.to_string(),
                now,
                domain_id.to_string(),
            ],
        )?;

        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            None,
            None,
            Vec::new(),
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    pub fn reserve_mail_domain_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        tenant_id: Uuid,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainDelete,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        if existing.state == MailResourceState::Deleting {
            return Err(StoreError::MailOperationInProgress);
        }

        let active_mailboxes: i64 = tx.query_row(
            "SELECT COUNT(*) FROM mail_mailboxes WHERE domain_id = ?1 AND active = 1",
            [domain_id.to_string()],
            |row| row.get(0),
        )?;
        if active_mailboxes > 0 {
            return Err(StoreError::MailConflict(
                "Cannot delete mail domain with active mailboxes".to_string(),
            ));
        }

        let active_aliases: i64 = tx.query_row(
            "SELECT COUNT(*) FROM mail_aliases WHERE domain_id = ?1 AND active = 1",
            [domain_id.to_string()],
            |row| row.get(0),
        )?;
        if active_aliases > 0 {
            return Err(StoreError::MailConflict(
                "Cannot delete mail domain with active aliases".to_string(),
            ));
        }

        let request_id = Uuid::new_v4();
        let revision = existing.desired_revision.saturating_add(1);
        tx.execute(
            "UPDATE mail_domains
             SET state = 'deleting', desired_revision = ?1, queued_revision = ?1,
                 pending_job_id = NULL, last_error = NULL, operation_request_id = ?2, updated_at = ?3
             WHERE domain_id = ?4",
            params![
                revision as i64,
                request_id.to_string(),
                now,
                domain_id.to_string(),
            ],
        )?;

        let domain_rec = load_domain_tx(&tx, domain_id)?;
        let reservation = MailOperationReservation {
            domain: domain_rec,
            mailboxes: Vec::new(),
            aliases: Vec::new(),
            dkim: None,
            target_mailbox: None,
            target_alias: None,
            request_id,
            revision,
            job_id: None,
            idempotent_replay: false,
            mailbox_secrets: Vec::new(),
        };
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainDelete,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_mail_mailbox_create(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        tenant_id: Uuid,
        address: &str,
        local_part: &str,
        quota_bytes: u64,
        active: bool,
        password_hash: &str,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing)?;

        let dup_mailbox: Option<String> = tx
            .query_row(
                "SELECT mailbox_id FROM mail_mailboxes WHERE address = ?1",
                [address],
                |row| row.get(0),
            )
            .optional()?;
        if dup_mailbox.is_some() {
            return Err(StoreError::MailConflict(format!(
                "Mailbox '{address}' already exists"
            )));
        }

        let mailbox_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO mail_mailboxes (
                mailbox_id, domain_id, tenant_id, address, local_part, quota_bytes, active, password_hash, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?9)",
            params![
                mailbox_id.to_string(),
                domain_id.to_string(),
                tenant_id.to_string(),
                address,
                local_part,
                quota_bytes as i64,
                i64::from(active),
                password_hash,
                now,
            ],
        )?;

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing, now)?;
        let target = load_mailbox_tx(&tx, mailbox_id)?;
        let secrets = load_mailbox_secrets_tx(&tx, domain_id)?;
        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            Some(target),
            None,
            secrets,
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_mail_mailbox_update(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        mailbox_id: Uuid,
        tenant_id: Uuid,
        quota_bytes: Option<u64>,
        active: Option<bool>,
        password_hash: Option<&str>,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing_domain) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing_domain)?;

        let existing_mailbox: Option<(u64, bool, String)> = tx
            .query_row(
                "SELECT quota_bytes, active, password_hash FROM mail_mailboxes
                 WHERE mailbox_id = ?1 AND domain_id = ?2 AND tenant_id = ?3",
                params![
                    mailbox_id.to_string(),
                    domain_id.to_string(),
                    tenant_id.to_string()
                ],
                |row| {
                    Ok((
                        row.get::<_, i64>(0)? as u64,
                        row.get::<_, i64>(1)? != 0,
                        row.get::<_, String>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((cur_quota, cur_active, cur_hash)) = existing_mailbox else {
            return Ok(None);
        };

        let new_quota = quota_bytes.unwrap_or(cur_quota);
        let new_active = active.unwrap_or(cur_active);
        let new_hash = password_hash.unwrap_or(&cur_hash);

        tx.execute(
            "UPDATE mail_mailboxes
             SET quota_bytes = ?1, active = ?2, password_hash = ?3, updated_at = ?4
             WHERE mailbox_id = ?5",
            params![
                new_quota as i64,
                i64::from(new_active),
                new_hash,
                now,
                mailbox_id.to_string(),
            ],
        )?;

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing_domain, now)?;
        let target = load_mailbox_tx(&tx, mailbox_id)?;
        let secrets = load_mailbox_secrets_tx(&tx, domain_id)?;
        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            Some(target),
            None,
            secrets,
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    pub fn reserve_mail_mailbox_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        mailbox_id: Uuid,
        tenant_id: Uuid,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing_domain) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing_domain)?;

        let Some(target) = tx
            .query_row(
                "SELECT mailbox_id, domain_id, tenant_id, address, local_part, quota_bytes, active, created_at, updated_at
                 FROM mail_mailboxes
                 WHERE mailbox_id = ?1 AND domain_id = ?2 AND tenant_id = ?3",
                params![
                    mailbox_id.to_string(),
                    domain_id.to_string(),
                    tenant_id.to_string()
                ],
                row_to_mailbox,
            )
            .optional()?
        else {
            return Ok(None);
        };

        // Prevent deleting a mailbox that is targeted by any alias owned by this tenant.
        let mut stmt = tx.prepare(
            "SELECT source, destinations_json FROM mail_aliases WHERE tenant_id = ?1",
        )?;
        let alias_rows = stmt.query_map([tenant_id.to_string()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })?;
        for row in alias_rows {
            let (source, dests_json) = row?;
            let dests: Vec<String> = serde_json::from_str(&dests_json)?;
            if dests.iter().any(|d| d == &target.address) {
                return Err(StoreError::MailConflict(format!(
                    "Mailbox '{}' is referenced by alias '{}'",
                    target.address, source
                )));
            }
        }
        drop(stmt);

        tx.execute(
            "DELETE FROM mail_mailboxes WHERE mailbox_id = ?1",
            [mailbox_id.to_string()],
        )?;

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing_domain, now)?;
        let secrets = load_mailbox_secrets_tx(&tx, domain_id)?;
        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            Some(target),
            None,
            secrets,
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_mail_alias_create(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        tenant_id: Uuid,
        source: &str,
        destinations: &[String],
        active: bool,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing_domain) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing_domain)?;

        let dup_alias: Option<String> = tx
            .query_row(
                "SELECT alias_id FROM mail_aliases WHERE source = ?1",
                [source],
                |row| row.get(0),
            )
            .optional()?;
        if dup_alias.is_some() {
            return Err(StoreError::MailConflict(format!(
                "Alias '{source}' already exists"
            )));
        }

        validate_local_alias_destinations(&tx, tenant_id, destinations)?;

        let alias_id = Uuid::new_v4();
        let dests_json = serde_json::to_string(destinations)?;
        tx.execute(
            "INSERT INTO mail_aliases (alias_id, domain_id, tenant_id, source, destinations_json, active, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?7)",
            params![
                alias_id.to_string(),
                domain_id.to_string(),
                tenant_id.to_string(),
                source,
                dests_json,
                i64::from(active),
                now,
            ],
        )?;

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing_domain, now)?;
        let target = load_alias_tx(&tx, alias_id)?;
        let secrets = load_mailbox_secrets_tx(&tx, domain_id)?;
        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            None,
            Some(target),
            secrets,
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_mail_alias_update(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        alias_id: Uuid,
        tenant_id: Uuid,
        destinations: Option<&[String]>,
        active: Option<bool>,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing_domain) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing_domain)?;

        let Some(existing_alias) = tx
            .query_row(
                "SELECT alias_id, domain_id, tenant_id, source, destinations_json, active, created_at, updated_at
                 FROM mail_aliases
                 WHERE alias_id = ?1 AND domain_id = ?2 AND tenant_id = ?3",
                params![
                    alias_id.to_string(),
                    domain_id.to_string(),
                    tenant_id.to_string()
                ],
                row_to_alias,
            )
            .optional()?
        else {
            return Ok(None);
        };

        let new_dests = match destinations {
            Some(d) => {
                validate_local_alias_destinations(&tx, tenant_id, d)?;
                d.to_vec()
            }
            None => existing_alias.destinations,
        };
        let new_active = active.unwrap_or(existing_alias.active);
        let dests_json = serde_json::to_string(&new_dests)?;

        tx.execute(
            "UPDATE mail_aliases SET destinations_json = ?1, active = ?2, updated_at = ?3 WHERE alias_id = ?4",
            params![dests_json, i64::from(new_active), now, alias_id.to_string()],
        )?;

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing_domain, now)?;
        let target = load_alias_tx(&tx, alias_id)?;
        let secrets = load_mailbox_secrets_tx(&tx, domain_id)?;
        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            None,
            Some(target),
            secrets,
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    pub fn reserve_mail_alias_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        alias_id: Uuid,
        tenant_id: Uuid,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing_domain) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing_domain)?;

        let Some(target) = tx
            .query_row(
                "SELECT alias_id, domain_id, tenant_id, source, destinations_json, active, created_at, updated_at
                 FROM mail_aliases
                 WHERE alias_id = ?1 AND domain_id = ?2 AND tenant_id = ?3",
                params![
                    alias_id.to_string(),
                    domain_id.to_string(),
                    tenant_id.to_string()
                ],
                row_to_alias,
            )
            .optional()?
        else {
            return Ok(None);
        };

        tx.execute(
            "DELETE FROM mail_aliases WHERE alias_id = ?1",
            [alias_id.to_string()],
        )?;

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing_domain, now)?;
        let secrets = load_mailbox_secrets_tx(&tx, domain_id)?;
        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            None,
            Some(target),
            secrets,
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DomainApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_mail_dkim_upsert(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        tenant_id: Uuid,
        selector: &str,
        enabled: bool,
        dns_zone_id: Option<Uuid>,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DkimApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing_domain) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing_domain)?;

        let effective_zone = dns_zone_id.or(existing_domain.dns_zone_id);
        if let Some(zone_id) = effective_zone {
            validate_dns_zone_ownership(&tx, zone_id, tenant_id, &existing_domain.domain)?;
        }

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing_domain, now)?;
        let existing_dkim = load_domain_dkim_tx(&tx, domain_id)?;
        let created_at = existing_dkim
            .as_ref()
            .map(|d| d.created_at.clone())
            .unwrap_or_else(|| now.to_string());

        // Clear txt_record/public_key_pem if selector changed so stale key material isn't shown.
        let selector_changed = existing_dkim
            .as_ref()
            .map(|d| d.selector != selector)
            .unwrap_or(true);
        let (txt_record, public_key_pem) = if selector_changed {
            (None, None)
        } else {
            let d = existing_dkim.as_ref().unwrap();
            (d.txt_record.clone(), d.public_key_pem.clone())
        };

        tx.execute(
            "INSERT INTO mail_dkim_keys (
                domain_id, tenant_id, selector, enabled, dns_zone_id, public_key_pem, txt_record,
                state, queued_revision, applied_revision, failed_revision, pending_job_id,
                last_error, operation_request_id, created_at, updated_at
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, 'provisioning', ?8, NULL, NULL, NULL, NULL, ?9, ?10, ?11)
             ON CONFLICT(domain_id) DO UPDATE SET
                selector = excluded.selector,
                enabled = excluded.enabled,
                dns_zone_id = excluded.dns_zone_id,
                public_key_pem = excluded.public_key_pem,
                txt_record = excluded.txt_record,
                state = 'provisioning',
                queued_revision = excluded.queued_revision,
                pending_job_id = NULL,
                last_error = NULL,
                operation_request_id = excluded.operation_request_id,
                updated_at = excluded.updated_at",
            params![
                domain_id.to_string(),
                tenant_id.to_string(),
                selector,
                i64::from(enabled),
                effective_zone.map(|id| id.to_string()),
                public_key_pem,
                txt_record,
                revision as i64,
                request_id.to_string(),
                created_at,
                now,
            ],
        )?;

        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            None,
            None,
            Vec::new(),
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DkimApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    pub fn reserve_mail_dkim_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        domain_id: Uuid,
        tenant_id: Uuid,
        now: &str,
    ) -> Result<Option<MailOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) = find_existing_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DkimApply,
        )? {
            tx.commit()?;
            return Ok(Some(replay));
        }

        let Some(existing_domain) = load_domain_for_tenant_tx(&tx, domain_id, tenant_id)? else {
            return Ok(None);
        };
        ensure_domain_mutable(&existing_domain)?;

        let Some(_existing_dkim) = load_domain_dkim_tx(&tx, domain_id)? else {
            return Ok(None);
        };

        let (request_id, revision) = bump_domain_for_apply(&tx, &existing_domain, now)?;
        tx.execute(
            "UPDATE mail_dkim_keys
             SET enabled = 0, state = 'deleting', queued_revision = ?1, pending_job_id = NULL,
                 last_error = NULL, operation_request_id = ?2, updated_at = ?3
             WHERE domain_id = ?4",
            params![
                revision as i64,
                request_id.to_string(),
                now,
                domain_id.to_string(),
            ],
        )?;

        let reservation = build_apply_reservation(
            &tx,
            domain_id,
            request_id,
            revision,
            None,
            None,
            Vec::new(),
        )?;
        insert_mail_operation(
            &tx,
            idempotency_key,
            fingerprint,
            MailOperation::DkimApply,
            domain_id,
            request_id,
            revision,
            &reservation,
        )?;
        tx.commit()?;
        Ok(Some(reservation))
    }

    pub fn set_mail_operation_job_id(
        &mut self,
        idempotency_key: &str,
        domain_id: Uuid,
        op: MailOperation,
        job_id: Uuid,
    ) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        tx.execute(
            "UPDATE mail_operations SET job_id = ?1 WHERE idempotency_key = ?2",
            params![job_id.to_string(), idempotency_key],
        )?;
        tx.execute(
            "UPDATE mail_domains SET pending_job_id = ?1 WHERE domain_id = ?2 AND queued_revision IS NOT NULL",
            params![job_id.to_string(), domain_id.to_string()],
        )?;
        if op == MailOperation::DkimApply {
            tx.execute(
                "UPDATE mail_dkim_keys SET pending_job_id = ?1 WHERE domain_id = ?2 AND queued_revision IS NOT NULL",
                params![job_id.to_string(), domain_id.to_string()],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn cancel_mail_operation_reservation(
        &mut self,
        idempotency_key: &str,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "DELETE FROM mail_operations WHERE idempotency_key = ?1",
            [idempotency_key],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_mail_operation(
        &mut self,
        request_id: Uuid,
        operation: MailOperation,
        succeeded: bool,
        reported_revision: Option<u64>,
        dkim_txt_record: Option<&str>,
        dkim_public_key_pem: Option<&str>,
        message: Option<&str>,
        now: &str,
    ) -> Result<bool, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let op_row: Option<(String, i64)> = tx
            .query_row(
                "SELECT domain_id, revision FROM mail_operations WHERE request_id = ?1 AND operation = ?2",
                params![request_id.to_string(), operation.as_str()],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((domain_id_str, op_revision_i64)) = op_row else {
            return Ok(false);
        };
        let op_revision = op_revision_i64 as u64;

        // If the agent reported a revision, ensure it matches the reserved revision.
        if let Some(rev) = reported_revision {
            if rev != op_revision {
                tx.commit()?;
                return Ok(false);
            }
        }

        let active_req: Option<(String, Option<i64>)> = tx
            .query_row(
                "SELECT operation_request_id, applied_revision FROM mail_domains WHERE domain_id = ?1",
                [&domain_id_str],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((active_request_id, applied_rev)) = active_req else {
            tx.commit()?;
            return Ok(false);
        };

        // Ignore obsolete/superseded completions.
        if active_request_id != request_id.to_string()
            || applied_rev.map(|r| op_revision < r as u64).unwrap_or(false)
        {
            tx.commit()?;
            return Ok(false);
        }

        match operation {
            MailOperation::DomainApply => {
                if succeeded {
                    tx.execute(
                        "UPDATE mail_domains
                         SET state = 'ready', applied_revision = ?1, queued_revision = NULL,
                             pending_job_id = NULL, last_error = NULL, updated_at = ?2
                         WHERE domain_id = ?3",
                        params![op_revision as i64, now, domain_id_str],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE mail_domains
                         SET state = 'failed', failed_revision = ?1, queued_revision = NULL,
                             pending_job_id = NULL, last_error = ?2, updated_at = ?3
                         WHERE domain_id = ?4",
                        params![
                            op_revision as i64,
                            message.unwrap_or("mail.domain.apply failed"),
                            now,
                            domain_id_str,
                        ],
                    )?;
                }
            }
            MailOperation::DkimApply => {
                if succeeded {
                    tx.execute(
                        "UPDATE mail_domains
                         SET state = 'ready', applied_revision = ?1, queued_revision = NULL,
                             pending_job_id = NULL, last_error = NULL, updated_at = ?2
                         WHERE domain_id = ?3",
                        params![op_revision as i64, now, domain_id_str],
                    )?;
                    let dkim_deleting: Option<String> = tx
                        .query_row(
                            "SELECT state FROM mail_dkim_keys WHERE domain_id = ?1",
                            [&domain_id_str],
                            |row| row.get(0),
                        )
                        .optional()?;
                    if dkim_deleting.as_deref() == Some("deleting") {
                        tx.execute(
                            "DELETE FROM mail_dkim_keys WHERE domain_id = ?1",
                            [&domain_id_str],
                        )?;
                    } else {
                        tx.execute(
                            "UPDATE mail_dkim_keys
                             SET state = 'ready', applied_revision = ?1, queued_revision = NULL,
                                 pending_job_id = NULL, last_error = NULL,
                                 txt_record = COALESCE(?2, txt_record),
                                 public_key_pem = COALESCE(?3, public_key_pem),
                                 updated_at = ?4
                             WHERE domain_id = ?5",
                            params![
                                op_revision as i64,
                                dkim_txt_record,
                                dkim_public_key_pem,
                                now,
                                domain_id_str,
                            ],
                        )?;
                    }
                } else {
                    let err = message.unwrap_or("mail.dkim.apply failed");
                    tx.execute(
                        "UPDATE mail_domains
                         SET state = 'failed', failed_revision = ?1, queued_revision = NULL,
                             pending_job_id = NULL, last_error = ?2, updated_at = ?3
                         WHERE domain_id = ?4",
                        params![op_revision as i64, err, now, domain_id_str],
                    )?;
                    tx.execute(
                        "UPDATE mail_dkim_keys
                         SET state = 'failed', failed_revision = ?1, queued_revision = NULL,
                             pending_job_id = NULL, last_error = ?2, updated_at = ?3
                         WHERE domain_id = ?4",
                        params![op_revision as i64, err, now, domain_id_str],
                    )?;
                }
            }
            MailOperation::DomainDelete => {
                if succeeded {
                    tx.execute(
                        "DELETE FROM mail_dkim_keys WHERE domain_id = ?1",
                        [&domain_id_str],
                    )?;
                    tx.execute(
                        "DELETE FROM mail_aliases WHERE domain_id = ?1",
                        [&domain_id_str],
                    )?;
                    tx.execute(
                        "DELETE FROM mail_mailboxes WHERE domain_id = ?1",
                        [&domain_id_str],
                    )?;
                    tx.execute(
                        "DELETE FROM mail_domains WHERE domain_id = ?1",
                        [&domain_id_str],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE mail_domains
                         SET state = 'failed', failed_revision = ?1, queued_revision = NULL,
                             pending_job_id = NULL, last_error = ?2, updated_at = ?3
                         WHERE domain_id = ?4",
                        params![
                            op_revision as i64,
                            message.unwrap_or("mail.domain.delete failed"),
                            now,
                            domain_id_str,
                        ],
                    )?;
                }
            }
        }

        tx.commit()?;
        Ok(true)
    }
}

fn domain_owned_by_tenant(
    conn: &rusqlite::Connection,
    domain_id: Uuid,
    tenant_id: Uuid,
) -> Result<bool, StoreError> {
    let exists: Option<String> = conn
        .query_row(
            "SELECT domain_id FROM mail_domains WHERE domain_id = ?1 AND tenant_id = ?2",
            params![domain_id.to_string(), tenant_id.to_string()],
            |row| row.get(0),
        )
        .optional()?;
    Ok(exists.is_some())
}

fn ensure_domain_mutable(domain: &MailDomainRecord) -> Result<(), StoreError> {
    if domain.state == MailResourceState::Deleting {
        return Err(StoreError::MailConflict(
            "Mail domain is being deleted".to_string(),
        ));
    }
    Ok(())
}

fn validate_dns_zone_ownership(
    tx: &Transaction<'_>,
    zone_id: Uuid,
    tenant_id: Uuid,
    mail_domain: &str,
) -> Result<(), StoreError> {
    let zone: Option<(String, String)> = tx
        .query_row(
            "SELECT tenant_id, domain FROM dns_zones WHERE zone_id = ?1",
            [zone_id.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()?;
    let Some((zone_tenant_str, zone_domain)) = zone else {
        return Err(StoreError::NotFound(zone_id));
    };
    let zone_tenant = Uuid::parse_str(&zone_tenant_str)
        .map_err(|e| StoreError::Parse(format!("invalid dns zone tenant_id: {e}")))?;
    if zone_tenant != tenant_id {
        return Err(StoreError::MailConflict(
            "Linked DNS zone belongs to another tenant".to_string(),
        ));
    }
    if zone_domain != mail_domain {
        return Err(StoreError::MailConflict(format!(
            "Linked DNS zone domain '{zone_domain}' does not match mail domain '{mail_domain}'"
        )));
    }
    Ok(())
}

fn validate_local_alias_destinations(
    tx: &Transaction<'_>,
    tenant_id: Uuid,
    destinations: &[String],
) -> Result<(), StoreError> {
    for dest in destinations {
        let owner: Option<(String, i64)> = tx
            .query_row(
                "SELECT tenant_id, active FROM mail_mailboxes WHERE address = ?1",
                [dest],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?;
        let Some((owner_tenant_str, active)) = owner else {
            return Err(StoreError::MailConflict(format!(
                "Alias destination '{dest}' is not a local mailbox"
            )));
        };
        let owner_tenant = Uuid::parse_str(&owner_tenant_str)
            .map_err(|e| StoreError::Parse(format!("invalid mailbox tenant_id: {e}")))?;
        if owner_tenant != tenant_id {
            return Err(StoreError::MailConflict(format!(
                "Alias destination '{dest}' belongs to another tenant"
            )));
        }
        if active == 0 {
            return Err(StoreError::MailConflict(format!(
                "Alias destination mailbox '{dest}' is inactive"
            )));
        }
    }
    Ok(())
}

fn bump_domain_for_apply(
    tx: &Transaction<'_>,
    existing: &MailDomainRecord,
    now: &str,
) -> Result<(Uuid, u64), StoreError> {
    let request_id = Uuid::new_v4();
    let revision = existing.desired_revision.saturating_add(1);
    tx.execute(
        "UPDATE mail_domains
         SET state = 'provisioning', desired_revision = ?1, queued_revision = ?1,
             pending_job_id = NULL, last_error = NULL, operation_request_id = ?2, updated_at = ?3
         WHERE domain_id = ?4",
        params![
            revision as i64,
            request_id.to_string(),
            now,
            existing.id.to_string(),
        ],
    )?;
    Ok((request_id, revision))
}

fn build_apply_reservation(
    tx: &Transaction<'_>,
    domain_id: Uuid,
    request_id: Uuid,
    revision: u64,
    target_mailbox: Option<MailboxRecord>,
    target_alias: Option<MailAliasRecord>,
    mailbox_secrets: Vec<(Uuid, String)>,
) -> Result<MailOperationReservation, StoreError> {
    let domain = load_domain_tx(tx, domain_id)?;
    let mailboxes = load_domain_mailboxes(tx, domain_id)?;
    let aliases = load_domain_aliases(tx, domain_id)?;
    let dkim = load_domain_dkim_tx(tx, domain_id)?;
    Ok(MailOperationReservation {
        domain,
        mailboxes,
        aliases,
        dkim,
        target_mailbox,
        target_alias,
        request_id,
        revision,
        job_id: None,
        idempotent_replay: false,
        mailbox_secrets,
    })
}

fn load_domain_tx(tx: &Transaction<'_>, domain_id: Uuid) -> Result<MailDomainRecord, StoreError> {
    tx.query_row(
        "SELECT domain_id, tenant_id, node_id, domain, dns_zone_id, active, state,
                desired_revision, queued_revision, applied_revision, failed_revision,
                pending_job_id, last_error, created_at, updated_at
         FROM mail_domains WHERE domain_id = ?1",
        [domain_id.to_string()],
        row_to_mail_domain,
    )
    .optional()?
    .ok_or(StoreError::NotFound(domain_id))
}

fn load_domain_for_tenant_tx(
    tx: &Transaction<'_>,
    domain_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<MailDomainRecord>, StoreError> {
    tx.query_row(
        "SELECT domain_id, tenant_id, node_id, domain, dns_zone_id, active, state,
                desired_revision, queued_revision, applied_revision, failed_revision,
                pending_job_id, last_error, created_at, updated_at
         FROM mail_domains WHERE domain_id = ?1 AND tenant_id = ?2",
        params![domain_id.to_string(), tenant_id.to_string()],
        row_to_mail_domain,
    )
    .optional()
    .map_err(StoreError::from)
}

fn load_mailbox_tx(tx: &Transaction<'_>, mailbox_id: Uuid) -> Result<MailboxRecord, StoreError> {
    tx.query_row(
        "SELECT mailbox_id, domain_id, tenant_id, address, local_part, quota_bytes, active, created_at, updated_at
         FROM mail_mailboxes WHERE mailbox_id = ?1",
        [mailbox_id.to_string()],
        row_to_mailbox,
    )
    .optional()?
    .ok_or(StoreError::NotFound(mailbox_id))
}

fn load_alias_tx(tx: &Transaction<'_>, alias_id: Uuid) -> Result<MailAliasRecord, StoreError> {
    tx.query_row(
        "SELECT alias_id, domain_id, tenant_id, source, destinations_json, active, created_at, updated_at
         FROM mail_aliases WHERE alias_id = ?1",
        [alias_id.to_string()],
        row_to_alias,
    )
    .optional()?
    .ok_or(StoreError::NotFound(alias_id))
}

fn load_domain_mailboxes(
    conn: &rusqlite::Connection,
    domain_id: Uuid,
) -> Result<Vec<MailboxRecord>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT mailbox_id, domain_id, tenant_id, address, local_part, quota_bytes, active, created_at, updated_at
         FROM mail_mailboxes WHERE domain_id = ?1 ORDER BY address ASC, mailbox_id ASC",
    )?;
    let rows = stmt.query_map([domain_id.to_string()], row_to_mailbox)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn load_mailbox_secrets_tx(
    tx: &Transaction<'_>,
    domain_id: Uuid,
) -> Result<Vec<(Uuid, String)>, StoreError> {
    let mut stmt = tx.prepare(
        "SELECT mailbox_id, password_hash FROM mail_mailboxes WHERE domain_id = ?1 ORDER BY address ASC",
    )?;
    let rows = stmt.query_map([domain_id.to_string()], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id_str, hash) = row?;
        let id = Uuid::parse_str(&id_str)
            .map_err(|e| StoreError::Parse(format!("invalid mailbox_id: {e}")))?;
        out.push((id, hash));
    }
    Ok(out)
}

fn load_domain_aliases(
    conn: &rusqlite::Connection,
    domain_id: Uuid,
) -> Result<Vec<MailAliasRecord>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT alias_id, domain_id, tenant_id, source, destinations_json, active, created_at, updated_at
         FROM mail_aliases WHERE domain_id = ?1 ORDER BY source ASC, alias_id ASC",
    )?;
    let rows = stmt.query_map([domain_id.to_string()], row_to_alias)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

fn load_domain_dkim(
    conn: &rusqlite::Connection,
    domain_id: Uuid,
) -> Result<Option<MailDkimRecord>, StoreError> {
    conn.query_row(
        "SELECT domain_id, tenant_id, selector, enabled, dns_zone_id, txt_record, public_key_pem,
                state, queued_revision, applied_revision, failed_revision, pending_job_id,
                last_error, created_at, updated_at
         FROM mail_dkim_keys WHERE domain_id = ?1",
        [domain_id.to_string()],
        row_to_dkim,
    )
    .optional()
    .map_err(StoreError::from)
}

fn load_domain_dkim_tx(
    tx: &Transaction<'_>,
    domain_id: Uuid,
) -> Result<Option<MailDkimRecord>, StoreError> {
    load_domain_dkim(tx, domain_id)
}

fn find_existing_mail_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    expected_op: MailOperation,
) -> Result<Option<MailOperationReservation>, StoreError> {
    let row: Option<(String, String, String, Option<String>)> = tx
        .query_row(
            "SELECT fingerprint, operation, snapshot, job_id FROM mail_operations WHERE idempotency_key = ?1",
            [idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?;
    let Some((stored_fp, stored_op, snapshot_json, job_id_str)) = row else {
        return Ok(None);
    };
    if stored_fp != fingerprint || stored_op != expected_op.as_str() {
        return Err(StoreError::IdempotencyConflict);
    }
    let mut reservation: MailOperationReservation = serde_json::from_str(&snapshot_json)?;
    reservation.idempotent_replay = true;
    reservation.job_id = job_id_str
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| StoreError::Parse(format!("invalid mail job_id: {e}")))?;
    Ok(Some(reservation))
}

#[allow(clippy::too_many_arguments)]
fn insert_mail_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    operation: MailOperation,
    domain_id: Uuid,
    request_id: Uuid,
    revision: u64,
    snapshot: &MailOperationReservation,
) -> Result<(), StoreError> {
    let snapshot_json = serde_json::to_string(snapshot)?;
    tx.execute(
        "INSERT INTO mail_operations (idempotency_key, fingerprint, operation, domain_id, request_id, revision, snapshot, job_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
        params![
            idempotency_key,
            fingerprint,
            operation.as_str(),
            domain_id.to_string(),
            request_id.to_string(),
            revision as i64,
            snapshot_json,
        ],
    )?;
    Ok(())
}

fn parse_uuid_col(row: &Row<'_>, idx: usize) -> Result<Uuid, rusqlite::Error> {
    let raw: String = row.get(idx)?;
    Uuid::parse_str(&raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
    })
}

fn parse_opt_uuid_col(row: &Row<'_>, idx: usize) -> Result<Option<Uuid>, rusqlite::Error> {
    let raw: Option<String> = row.get(idx)?;
    raw.map(|s| {
        Uuid::parse_str(&s).map_err(|e| {
            rusqlite::Error::FromSqlConversionFailure(idx, rusqlite::types::Type::Text, Box::new(e))
        })
    })
    .transpose()
}

fn row_to_mail_domain(row: &Row<'_>) -> Result<MailDomainRecord, rusqlite::Error> {
    let state_str: String = row.get(6)?;
    let state = MailResourceState::parse(&state_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(MailDomainRecord {
        id: parse_uuid_col(row, 0)?,
        tenant_id: parse_uuid_col(row, 1)?,
        node_id: parse_uuid_col(row, 2)?,
        domain: row.get(3)?,
        dns_zone_id: parse_opt_uuid_col(row, 4)?,
        active: row.get::<_, i64>(5)? != 0,
        state,
        desired_revision: row.get::<_, i64>(7)? as u64,
        queued_revision: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
        applied_revision: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
        failed_revision: row.get::<_, Option<i64>>(10)?.map(|v| v as u64),
        pending_job_id: parse_opt_uuid_col(row, 11)?,
        last_error: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

fn row_to_mailbox(row: &Row<'_>) -> Result<MailboxRecord, rusqlite::Error> {
    Ok(MailboxRecord {
        id: parse_uuid_col(row, 0)?,
        domain_id: parse_uuid_col(row, 1)?,
        tenant_id: parse_uuid_col(row, 2)?,
        address: row.get(3)?,
        local_part: row.get(4)?,
        quota_bytes: row.get::<_, i64>(5)? as u64,
        active: row.get::<_, i64>(6)? != 0,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
    })
}

fn row_to_alias(row: &Row<'_>) -> Result<MailAliasRecord, rusqlite::Error> {
    let dests_json: String = row.get(4)?;
    let destinations: Vec<String> = serde_json::from_str(&dests_json).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(MailAliasRecord {
        id: parse_uuid_col(row, 0)?,
        domain_id: parse_uuid_col(row, 1)?,
        tenant_id: parse_uuid_col(row, 2)?,
        source: row.get(3)?,
        destinations,
        active: row.get::<_, i64>(5)? != 0,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn row_to_dkim(row: &Row<'_>) -> Result<MailDkimRecord, rusqlite::Error> {
    let state_str: String = row.get(7)?;
    let state = MailResourceState::parse(&state_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(7, rusqlite::types::Type::Text, Box::new(e))
    })?;
    Ok(MailDkimRecord {
        domain_id: parse_uuid_col(row, 0)?,
        tenant_id: parse_uuid_col(row, 1)?,
        selector: row.get(2)?,
        enabled: row.get::<_, i64>(3)? != 0,
        dns_zone_id: parse_opt_uuid_col(row, 4)?,
        txt_record: row.get(5)?,
        public_key_pem: row.get(6)?,
        state,
        queued_revision: row.get::<_, Option<i64>>(8)?.map(|v| v as u64),
        applied_revision: row.get::<_, Option<i64>>(9)?.map(|v| v as u64),
        failed_revision: row.get::<_, Option<i64>>(10)?.map(|v| v as u64),
        pending_job_id: parse_opt_uuid_col(row, 11)?,
        last_error: row.get(12)?,
        created_at: row.get(13)?,
        updated_at: row.get(14)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mail_domain_mailbox_alias_dkim_and_obsolete_completion_lifecycle() {
        let mut store = ControlStore::new_in_memory().expect("open store");
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let node_id = Uuid::new_v4();
        let now = "2026-11-27T12:00:00Z";

        let res1 = store
            .reserve_mail_domain_create(
                "mail-create-1",
                "fp-1",
                tenant_a,
                node_id,
                "example.com",
                None,
                true,
                now,
            )
            .unwrap();
        assert_eq!(res1.domain.desired_revision, 1);
        assert_eq!(res1.domain.queued_revision, Some(1));
        assert_eq!(res1.domain.applied_revision, None);

        // Exact replay works; conflicting replay returns IdempotencyConflict
        let replay = store
            .reserve_mail_domain_create(
                "mail-create-1",
                "fp-1",
                tenant_a,
                node_id,
                "example.com",
                None,
                true,
                now,
            )
            .unwrap();
        assert!(replay.idempotent_replay);
        assert!(matches!(
            store.reserve_mail_domain_create(
                "mail-create-1",
                "fp-different",
                tenant_a,
                node_id,
                "example.com",
                None,
                true,
                now,
            ),
            Err(StoreError::IdempotencyConflict)
        ));

        // Create mailbox
        let mb_res = store
            .reserve_mail_mailbox_create(
                "mb-create-1",
                "fp-mb-1",
                res1.domain.id,
                tenant_a,
                "alice@example.com",
                "alice",
                1024,
                true,
                "$argon2id$v=19$m=19456,t=2,p=1$salt$hash",
                now,
            )
            .unwrap()
            .unwrap();
        assert_eq!(mb_res.revision, 2);

        // Obsolete completion for rev 1 must be ignored
        let applied_old = store
            .complete_mail_operation(
                res1.request_id,
                MailOperation::DomainApply,
                true,
                Some(1),
                None,
                None,
                None,
                now,
            )
            .unwrap();
        assert!(!applied_old);

        // Current completion for rev 2 succeeds
        let applied_cur = store
            .complete_mail_operation(
                mb_res.request_id,
                MailOperation::DomainApply,
                true,
                Some(2),
                None,
                None,
                None,
                now,
            )
            .unwrap();
        assert!(applied_cur);
        let details = store
            .get_mail_domain_details(res1.domain.id, tenant_a)
            .unwrap()
            .unwrap();
        assert_eq!(details.domain.applied_revision, Some(2));
        assert_eq!(details.domain.queued_revision, None);

        // Alias to foreign tenant's domain/mailbox is rejected
        assert!(matches!(
            store.reserve_mail_alias_create(
                "alias-foreign",
                "fp-af",
                res1.domain.id,
                tenant_b,
                "help@example.com",
                &["alice@example.com".to_string()],
                true,
                now,
            ),
            Ok(None)
        ));

        // Deleting domain with active mailbox is rejected
        assert!(matches!(
            store.reserve_mail_domain_delete(
                "del-dom-1",
                "fp-del-1",
                res1.domain.id,
                tenant_a,
                now,
            ),
            Err(StoreError::MailConflict(_))
        ));
    }
}

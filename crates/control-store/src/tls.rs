#![forbid(unsafe_code)]

use super::{ControlStore, StoreError};
use crate::websites::WebsiteState;
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebsiteTlsMode {
    CustomPem,
    AcmeHttp01,
}

impl WebsiteTlsMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::CustomPem => "custom_pem",
            Self::AcmeHttp01 => "acme_http01",
        }
    }
}

impl FromStr for WebsiteTlsMode {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "custom_pem" => Ok(Self::CustomPem),
            "acme_http01" => Ok(Self::AcmeHttp01),
            other => Err(StoreError::Parse(format!("Unknown TLS mode: {other}"))),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebsiteTlsState {
    Disabled,
    Provisioning,
    Active,
    Failed,
    Deleting,
}

impl WebsiteTlsState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Provisioning => "provisioning",
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Deleting => "deleting",
        }
    }
}

impl FromStr for WebsiteTlsState {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "disabled" => Ok(Self::Disabled),
            "provisioning" => Ok(Self::Provisioning),
            "active" => Ok(Self::Active),
            "failed" => Ok(Self::Failed),
            "deleting" => Ok(Self::Deleting),
            other => Err(StoreError::Parse(format!("Unknown TLS state: {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebsiteTlsRecord {
    pub site_id: Uuid,
    pub tenant_id: Uuid,
    pub mode: WebsiteTlsMode,
    pub state: WebsiteTlsState,
    pub acme_directory_url: Option<String>,
    pub contact_email: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsiteTlsOperation {
    Apply,
    Delete,
}

impl WebsiteTlsOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "tls.apply",
            Self::Delete => "tls.delete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebsiteTlsOperationReservation {
    pub operation: WebsiteTlsOperation,
    pub request_id: Uuid,
    pub tls: WebsiteTlsRecord,
    pub job_id: Option<String>,
    pub created: bool,
}

impl ControlStore {
    pub fn get_website_tls_for_tenant(
        &self,
        site_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<WebsiteTlsRecord>, StoreError> {
        // Ensure site exists and belongs to tenant
        let _ = self.get_website_for_tenant(site_id, tenant_id)?;
        load_tls_for_tenant(&self.conn, site_id, tenant_id)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_website_tls_apply(
        &mut self,
        idempotency_key: &str,
        fingerprint: &Value,
        tenant_id: Uuid,
        site_id: Uuid,
        proposed_request_id: Uuid,
        mode: WebsiteTlsMode,
        acme_directory_url: Option<&str>,
        contact_email: Option<&str>,
        now: &str,
    ) -> Result<WebsiteTlsOperationReservation, StoreError> {
        let key = idempotency_key.trim();
        if key.is_empty() || key.len() > 255 {
            return Err(StoreError::IdempotencyConflict);
        }
        let fingerprint = serde_json::to_string(&serde_json::json!({
            "tenant_id": tenant_id,
            "request": fingerprint,
        }))?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(existing) = lookup_tls_operation(&tx, key)? {
            return validate_existing_tls_operation(
                existing,
                WebsiteTlsOperation::Apply,
                &fingerprint,
            );
        }

        let site_state: Option<String> = tx
            .query_row(
                "SELECT state FROM websites WHERE site_id = ?1 AND tenant_id = ?2",
                params![site_id.to_string(), tenant_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(site_state) = site_state else {
            return Err(StoreError::WebsiteNotFound(site_id));
        };
        if WebsiteState::from_str(&site_state)? != WebsiteState::Active {
            return Err(StoreError::WebsiteConflict);
        }

        let existing_tls = load_tls_tx(&tx, site_id)?;
        let previous_snapshot = existing_tls
            .as_ref()
            .map(serde_json::to_string)
            .transpose()?;

        if let Some(ref existing) = existing_tls {
            if matches!(
                existing.state,
                WebsiteTlsState::Provisioning | WebsiteTlsState::Deleting
            ) {
                return Err(StoreError::WebsiteOperationInProgress);
            }
        }

        let created_at = existing_tls
            .as_ref()
            .map(|t| t.created_at.clone())
            .unwrap_or_else(|| now.to_string());

        let record = WebsiteTlsRecord {
            site_id,
            tenant_id,
            mode,
            state: WebsiteTlsState::Provisioning,
            acme_directory_url: acme_directory_url.map(str::to_string),
            contact_email: contact_email.map(str::to_string),
            last_error: None,
            created_at,
            updated_at: now.to_string(),
        };

        upsert_tls_tx(
            &tx,
            &record,
            WebsiteTlsOperation::Apply,
            proposed_request_id,
        )?;
        insert_tls_operation_tx(
            &tx,
            key,
            &fingerprint,
            WebsiteTlsOperation::Apply,
            site_id,
            proposed_request_id,
            &record,
            previous_snapshot.as_deref(),
        )?;

        tx.commit()?;
        Ok(WebsiteTlsOperationReservation {
            operation: WebsiteTlsOperation::Apply,
            request_id: proposed_request_id,
            tls: record,
            job_id: None,
            created: true,
        })
    }

    pub fn reserve_website_tls_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &Value,
        tenant_id: Uuid,
        site_id: Uuid,
        proposed_request_id: Uuid,
        now: &str,
    ) -> Result<WebsiteTlsOperationReservation, StoreError> {
        let key = idempotency_key.trim();
        if key.is_empty() || key.len() > 255 {
            return Err(StoreError::IdempotencyConflict);
        }
        let fingerprint = serde_json::to_string(&serde_json::json!({
            "tenant_id": tenant_id,
            "request": fingerprint,
        }))?;
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(existing) = lookup_tls_operation(&tx, key)? {
            return validate_existing_tls_operation(
                existing,
                WebsiteTlsOperation::Delete,
                &fingerprint,
            );
        }

        let site_exists: Option<String> = tx
            .query_row(
                "SELECT site_id FROM websites WHERE site_id = ?1 AND tenant_id = ?2",
                params![site_id.to_string(), tenant_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        if site_exists.is_none() {
            return Err(StoreError::WebsiteNotFound(site_id));
        }
        let Some(existing_tls) = load_tls_for_tenant_tx(&tx, site_id, tenant_id)? else {
            return Err(StoreError::WebsiteConflict);
        };
        if matches!(
            existing_tls.state,
            WebsiteTlsState::Provisioning | WebsiteTlsState::Deleting
        ) {
            return Err(StoreError::WebsiteOperationInProgress);
        }
        if existing_tls.state == WebsiteTlsState::Disabled {
            return Err(StoreError::WebsiteConflict);
        }

        let previous_snapshot = serde_json::to_string(&existing_tls)?;
        let record = WebsiteTlsRecord {
            site_id,
            tenant_id,
            mode: existing_tls.mode,
            state: WebsiteTlsState::Deleting,
            acme_directory_url: existing_tls.acme_directory_url,
            contact_email: existing_tls.contact_email,
            last_error: None,
            created_at: existing_tls.created_at,
            updated_at: now.to_string(),
        };

        upsert_tls_tx(
            &tx,
            &record,
            WebsiteTlsOperation::Delete,
            proposed_request_id,
        )?;
        insert_tls_operation_tx(
            &tx,
            key,
            &fingerprint,
            WebsiteTlsOperation::Delete,
            site_id,
            proposed_request_id,
            &record,
            Some(&previous_snapshot),
        )?;

        tx.commit()?;
        Ok(WebsiteTlsOperationReservation {
            operation: WebsiteTlsOperation::Delete,
            request_id: proposed_request_id,
            tls: record,
            job_id: None,
            created: true,
        })
    }

    pub fn attach_website_tls_job_id(
        &mut self,
        idempotency_key: &str,
        job_id: &str,
    ) -> Result<(), StoreError> {
        let updated = self.conn.execute(
            "UPDATE website_tls_operations SET job_id = ?1 WHERE idempotency_key = ?2",
            params![job_id, idempotency_key],
        )?;
        if updated != 1 {
            return Err(StoreError::IdempotencyConflict);
        }
        Ok(())
    }

    pub fn cancel_website_tls_operation_reservation(
        &mut self,
        idempotency_key: &str,
        site_id: Uuid,
        request_id: Uuid,
    ) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let previous: Option<Option<String>> = tx
            .query_row(
                "SELECT previous_tls_snapshot FROM website_tls_operations WHERE idempotency_key = ?1 AND site_id = ?2 AND request_id = ?3 AND job_id IS NULL",
                params![idempotency_key, site_id.to_string(), request_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(previous) = previous else {
            return Ok(());
        };
        match previous {
            Some(snapshot) => {
                let record: WebsiteTlsRecord = serde_json::from_str(&snapshot)?;
                tx.execute(
                    "UPDATE website_tls SET mode = ?1, state = ?2, acme_directory_url = ?3, contact_email = ?4, last_error = ?5, last_operation = NULL, last_request_id = NULL, updated_at = ?6 WHERE site_id = ?7",
                    params![
                        record.mode.as_str(),
                        record.state.as_str(),
                        record.acme_directory_url,
                        record.contact_email,
                        record.last_error,
                        record.updated_at,
                        site_id.to_string()
                    ],
                )?;
            }
            None => {
                tx.execute(
                    "DELETE FROM website_tls WHERE site_id = ?1",
                    [site_id.to_string()],
                )?;
            }
        }
        tx.execute(
            "DELETE FROM website_tls_operations WHERE idempotency_key = ?1",
            [idempotency_key],
        )?;
        tx.commit()?;
        Ok(())
    }

    pub fn complete_website_tls_operation(
        &mut self,
        site_id: Uuid,
        request_id: Uuid,
        operation: WebsiteTlsOperation,
        succeeded: bool,
        error_text: Option<&str>,
        now: &str,
    ) -> Result<bool, StoreError> {
        let (expected_state, next_state, stored_error) = match (operation, succeeded) {
            (WebsiteTlsOperation::Apply, true) => {
                (WebsiteTlsState::Provisioning, WebsiteTlsState::Active, None)
            }
            (WebsiteTlsOperation::Apply, false) => (
                WebsiteTlsState::Provisioning,
                WebsiteTlsState::Failed,
                error_text.or(Some("TLS certificate could not be applied")),
            ),
            (WebsiteTlsOperation::Delete, true) => {
                (WebsiteTlsState::Deleting, WebsiteTlsState::Disabled, None)
            }
            (WebsiteTlsOperation::Delete, false) => (
                WebsiteTlsState::Deleting,
                WebsiteTlsState::Failed,
                error_text.or(Some("TLS certificate could not be removed")),
            ),
        };

        let changed = self.conn.execute(
            "UPDATE website_tls SET state = ?1, last_error = ?2, updated_at = ?3 WHERE site_id = ?4 AND state = ?5 AND last_operation = ?6 AND last_request_id = ?7",
            params![
                next_state.as_str(),
                stored_error,
                now,
                site_id.to_string(),
                expected_state.as_str(),
                operation.as_str(),
                request_id.to_string(),
            ],
        )?;
        Ok(changed == 1)
    }
}

struct ExistingTlsOperation {
    fingerprint: String,
    operation: String,
    request_id: String,
    tls_snapshot: String,
    job_id: Option<String>,
}

fn lookup_tls_operation(
    tx: &Transaction<'_>,
    key: &str,
) -> Result<Option<ExistingTlsOperation>, StoreError> {
    Ok(tx
        .query_row(
            "SELECT fingerprint, operation, request_id, tls_snapshot, job_id FROM website_tls_operations WHERE idempotency_key = ?1",
            [key],
            |row| {
                Ok(ExistingTlsOperation {
                    fingerprint: row.get(0)?,
                    operation: row.get(1)?,
                    request_id: row.get(2)?,
                    tls_snapshot: row.get(3)?,
                    job_id: row.get(4)?,
                })
            },
        )
        .optional()?)
}

fn validate_existing_tls_operation(
    existing: ExistingTlsOperation,
    expected_op: WebsiteTlsOperation,
    fingerprint: &str,
) -> Result<WebsiteTlsOperationReservation, StoreError> {
    if existing.operation != expected_op.as_str() || existing.fingerprint != fingerprint {
        return Err(StoreError::IdempotencyConflict);
    }
    let request_id =
        Uuid::parse_str(&existing.request_id).map_err(|e| StoreError::Parse(e.to_string()))?;
    let tls: WebsiteTlsRecord = serde_json::from_str(&existing.tls_snapshot)?;
    Ok(WebsiteTlsOperationReservation {
        operation: expected_op,
        request_id,
        tls,
        job_id: existing.job_id,
        created: false,
    })
}

fn upsert_tls_tx(
    tx: &Transaction<'_>,
    record: &WebsiteTlsRecord,
    operation: WebsiteTlsOperation,
    request_id: Uuid,
) -> Result<(), StoreError> {
    tx.execute(
        r#"
        INSERT INTO website_tls (
            site_id, tenant_id, mode, state, acme_directory_url, contact_email,
            last_error, last_operation, last_request_id, created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        ON CONFLICT(site_id) DO UPDATE SET
            mode = excluded.mode,
            state = excluded.state,
            acme_directory_url = excluded.acme_directory_url,
            contact_email = excluded.contact_email,
            last_error = excluded.last_error,
            last_operation = excluded.last_operation,
            last_request_id = excluded.last_request_id,
            updated_at = excluded.updated_at
        "#,
        params![
            record.site_id.to_string(),
            record.tenant_id.to_string(),
            record.mode.as_str(),
            record.state.as_str(),
            record.acme_directory_url,
            record.contact_email,
            record.last_error,
            operation.as_str(),
            request_id.to_string(),
            record.created_at,
            record.updated_at,
        ],
    )?;
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn insert_tls_operation_tx(
    tx: &Transaction<'_>,
    key: &str,
    fingerprint: &str,
    operation: WebsiteTlsOperation,
    site_id: Uuid,
    request_id: Uuid,
    record: &WebsiteTlsRecord,
    previous_snapshot: Option<&str>,
) -> Result<(), StoreError> {
    let snapshot = serde_json::to_string(record)?;
    tx.execute(
        r#"
        INSERT INTO website_tls_operations (
            idempotency_key, fingerprint, operation, site_id, request_id,
            tls_snapshot, job_id, previous_tls_snapshot
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, ?7)
        "#,
        params![
            key,
            fingerprint,
            operation.as_str(),
            site_id.to_string(),
            request_id.to_string(),
            snapshot,
            previous_snapshot,
        ],
    )?;
    Ok(())
}

fn load_tls_for_tenant(
    conn: &Connection,
    site_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<WebsiteTlsRecord>, StoreError> {
    let row = conn
        .query_row(
            "SELECT site_id, tenant_id, mode, state, acme_directory_url, contact_email, last_error, created_at, updated_at FROM website_tls WHERE site_id = ?1 AND tenant_id = ?2",
            params![site_id.to_string(), tenant_id.to_string()],
            parse_tls_row,
        )
        .optional()?;
    row.transpose()
}

fn load_tls_for_tenant_tx(
    tx: &Transaction<'_>,
    site_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<WebsiteTlsRecord>, StoreError> {
    let row = tx
        .query_row(
            "SELECT site_id, tenant_id, mode, state, acme_directory_url, contact_email, last_error, created_at, updated_at FROM website_tls WHERE site_id = ?1 AND tenant_id = ?2",
            params![site_id.to_string(), tenant_id.to_string()],
            parse_tls_row,
        )
        .optional()?;
    row.transpose()
}

fn load_tls_tx(
    tx: &Transaction<'_>,
    site_id: Uuid,
) -> Result<Option<WebsiteTlsRecord>, StoreError> {
    let row = tx
        .query_row(
            "SELECT site_id, tenant_id, mode, state, acme_directory_url, contact_email, last_error, created_at, updated_at FROM website_tls WHERE site_id = ?1",
            [site_id.to_string()],
            parse_tls_row,
        )
        .optional()?;
    row.transpose()
}

fn parse_tls_row(
    row: &rusqlite::Row<'_>,
) -> Result<Result<WebsiteTlsRecord, StoreError>, rusqlite::Error> {
    let site_id: String = row.get(0)?;
    let tenant_id: String = row.get(1)?;
    let mode: String = row.get(2)?;
    let state: String = row.get(3)?;
    let acme_directory_url: Option<String> = row.get(4)?;
    let contact_email: Option<String> = row.get(5)?;
    let last_error: Option<String> = row.get(6)?;
    let created_at: String = row.get(7)?;
    let updated_at: String = row.get(8)?;

    Ok((|| {
        Ok(WebsiteTlsRecord {
            site_id: Uuid::parse_str(&site_id).map_err(|e| StoreError::Parse(e.to_string()))?,
            tenant_id: Uuid::parse_str(&tenant_id).map_err(|e| StoreError::Parse(e.to_string()))?,
            mode: WebsiteTlsMode::from_str(&mode)?,
            state: WebsiteTlsState::from_str(&state)?,
            acme_directory_url,
            contact_email,
            last_error,
            created_at,
            updated_at,
        })
    })())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::websites::{WebsiteOperation, WebsiteSpec};

    #[test]
    fn tls_apply_complete_and_delete_lifecycle() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant_id = Uuid::new_v4();
        let site_id = Uuid::new_v4();
        let site_req = Uuid::new_v4();

        let res = store
            .reserve_website_create(
                "create-site-1",
                &serde_json::json!({ "domain": "tls.test" }),
                tenant_id,
                site_id,
                site_req,
                "tls.test",
                WebsiteSpec::Static {
                    document_root: "/srv/fusionpanel/sites/tls.test".to_string(),
                },
                "2026-11-17T00:00:00Z",
            )
            .unwrap();
        assert!(store
            .complete_website_operation(
                res.site.id,
                res.request_id,
                WebsiteOperation::Apply,
                true,
                None,
                "2026-11-17T00:00:01Z",
            )
            .unwrap());

        let tls_req = Uuid::new_v4();
        let fp = serde_json::json!({ "mode": "custom_pem" });
        let tls_res = store
            .reserve_website_tls_apply(
                "tls-apply-1",
                &fp,
                tenant_id,
                site_id,
                tls_req,
                WebsiteTlsMode::CustomPem,
                None,
                None,
                "2026-11-17T00:01:00Z",
            )
            .unwrap();
        assert!(tls_res.created);
        assert_eq!(tls_res.tls.state, WebsiteTlsState::Provisioning);

        // Idempotent replay returns same snapshot
        let replay = store
            .reserve_website_tls_apply(
                "tls-apply-1",
                &fp,
                tenant_id,
                site_id,
                Uuid::new_v4(),
                WebsiteTlsMode::CustomPem,
                None,
                None,
                "2026-11-17T00:01:01Z",
            )
            .unwrap();
        assert!(!replay.created);
        assert_eq!(replay.request_id, tls_req);

        assert!(store
            .complete_website_tls_operation(
                site_id,
                tls_req,
                WebsiteTlsOperation::Apply,
                true,
                None,
                "2026-11-17T00:01:05Z",
            )
            .unwrap());
        let active = store
            .get_website_tls_for_tenant(site_id, tenant_id)
            .unwrap()
            .unwrap();
        assert_eq!(active.state, WebsiteTlsState::Active);

        // Delete TLS
        let del_req = Uuid::new_v4();
        let del_res = store
            .reserve_website_tls_delete(
                "tls-del-1",
                &serde_json::json!({ "op": "delete" }),
                tenant_id,
                site_id,
                del_req,
                "2026-11-17T00:02:00Z",
            )
            .unwrap();
        assert_eq!(del_res.tls.state, WebsiteTlsState::Deleting);
        assert!(store
            .complete_website_tls_operation(
                site_id,
                del_req,
                WebsiteTlsOperation::Delete,
                true,
                None,
                "2026-11-17T00:02:05Z",
            )
            .unwrap());
        let disabled = store
            .get_website_tls_for_tenant(site_id, tenant_id)
            .unwrap()
            .unwrap();
        assert_eq!(disabled.state, WebsiteTlsState::Disabled);
    }
}

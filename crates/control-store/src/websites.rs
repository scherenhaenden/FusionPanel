#![forbid(unsafe_code)]

use super::{ControlStore, StoreError};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::str::FromStr;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum WebsiteSpec {
    Static { document_root: String },
    ReverseProxy { upstream_url: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WebsiteState {
    Provisioning,
    Active,
    Failed,
    Deleting,
}

impl WebsiteState {
    fn as_str(self) -> &'static str {
        match self {
            Self::Provisioning => "provisioning",
            Self::Active => "active",
            Self::Failed => "failed",
            Self::Deleting => "deleting",
        }
    }
}

impl FromStr for WebsiteState {
    type Err = StoreError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "provisioning" => Ok(Self::Provisioning),
            "active" => Ok(Self::Active),
            "failed" => Ok(Self::Failed),
            "deleting" => Ok(Self::Deleting),
            other => Err(StoreError::Parse(format!("Unknown website state: {other}"))),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WebsiteRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub domain: String,
    #[serde(flatten)]
    pub spec: WebsiteSpec,
    pub state: WebsiteState,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebsiteOperation {
    Apply,
    Delete,
}

impl WebsiteOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "site.apply",
            Self::Delete => "site.delete",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WebsiteOperationReservation {
    pub operation: WebsiteOperation,
    pub request_id: Uuid,
    /// Snapshot returned for this idempotent request, even after the live site changes.
    pub site: WebsiteRecord,
    pub job_id: Option<String>,
    pub created: bool,
}

struct RawWebsite {
    id: String,
    domain: String,
    spec: String,
    state: String,
    last_error: Option<String>,
    created_at: String,
    updated_at: String,
    request_id: String,
    job_id: Option<String>,
    tenant_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredWebsiteSnapshot {
    record: WebsiteRecord,
    request_id: String,
    job_id: Option<String>,
}

fn raw_website(row: &rusqlite::Row<'_>) -> rusqlite::Result<RawWebsite> {
    Ok(RawWebsite {
        id: row.get(0)?,
        domain: row.get(1)?,
        spec: row.get(2)?,
        state: row.get(3)?,
        last_error: row.get(4)?,
        created_at: row.get(5)?,
        updated_at: row.get(6)?,
        request_id: row.get(7)?,
        job_id: row.get(8)?,
        tenant_id: row.get(9)?,
    })
}

fn decode_stored_website(raw: RawWebsite) -> Result<StoredWebsiteSnapshot, StoreError> {
    let request_id = Uuid::parse_str(&raw.request_id)
        .map_err(|error| StoreError::Parse(error.to_string()))?
        .to_string();
    let job_id = raw.job_id.clone();
    Ok(StoredWebsiteSnapshot {
        record: decode_website(raw)?,
        request_id,
        job_id,
    })
}

fn decode_website(raw: RawWebsite) -> Result<WebsiteRecord, StoreError> {
    Ok(WebsiteRecord {
        id: Uuid::parse_str(&raw.id).map_err(|error| StoreError::Parse(error.to_string()))?,
        tenant_id: Uuid::parse_str(&raw.tenant_id)
            .map_err(|error| StoreError::Parse(error.to_string()))?,
        domain: raw.domain,
        spec: serde_json::from_str(&raw.spec)?,
        state: raw.state.parse()?,
        last_error: raw.last_error,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    })
}

fn get_by_id(conn: &Connection, site_id: Uuid) -> Result<Option<WebsiteRecord>, StoreError> {
    let raw = conn
        .query_row(
            "SELECT site_id, domain, spec, state, last_error, created_at, updated_at, operation_request_id, job_id, tenant_id FROM websites WHERE site_id = ?1",
            [site_id.to_string()],
            raw_website,
        )
        .optional()?;
    raw.map(decode_website).transpose()
}

fn get_by_id_for_tenant(
    conn: &Connection,
    site_id: Uuid,
    tenant_id: Uuid,
) -> Result<Option<WebsiteRecord>, StoreError> {
    let raw = conn
        .query_row(
            "SELECT site_id, domain, spec, state, last_error, created_at, updated_at, operation_request_id, job_id, tenant_id FROM websites WHERE site_id = ?1 AND tenant_id = ?2",
            params![site_id.to_string(), tenant_id.to_string()],
            raw_website,
        )
        .optional()?;
    raw.map(decode_website).transpose()
}

fn get_stored_by_domain(
    conn: &Connection,
    domain: &str,
) -> Result<Option<StoredWebsiteSnapshot>, StoreError> {
    let raw = conn
        .query_row(
            "SELECT site_id, domain, spec, state, last_error, created_at, updated_at, operation_request_id, job_id, tenant_id FROM websites WHERE domain = ?1 COLLATE NOCASE",
            [domain],
            raw_website,
        )
        .optional()?;
    raw.map(decode_stored_website).transpose()
}

fn get_stored_by_id(
    conn: &Connection,
    site_id: Uuid,
) -> Result<Option<StoredWebsiteSnapshot>, StoreError> {
    let raw = conn
        .query_row(
            "SELECT site_id, domain, spec, state, last_error, created_at, updated_at, operation_request_id, job_id, tenant_id FROM websites WHERE site_id = ?1",
            [site_id.to_string()],
            raw_website,
        )
        .optional()?;
    raw.map(decode_stored_website).transpose()
}

fn existing_reservation(
    tx: &Transaction<'_>,
    key: &str,
    fingerprint: &str,
    operation: WebsiteOperation,
) -> Result<Option<WebsiteOperationReservation>, StoreError> {
    let existing = tx
        .query_row(
            "SELECT fingerprint, operation, request_id, site_snapshot, job_id FROM website_operations WHERE idempotency_key = ?1",
            [key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;

    let Some((stored_fingerprint, stored_operation, request_id, site_snapshot, job_id)) = existing
    else {
        return Ok(None);
    };
    if stored_fingerprint != fingerprint || stored_operation != operation.as_str() {
        return Err(StoreError::IdempotencyConflict);
    }
    Ok(Some(WebsiteOperationReservation {
        operation,
        request_id: Uuid::parse_str(&request_id)
            .map_err(|error| StoreError::Parse(error.to_string()))?,
        site: serde_json::from_str(&site_snapshot)?,
        job_id,
        created: false,
    }))
}

fn save_reservation(
    tx: &Transaction<'_>,
    key: &str,
    fingerprint: &str,
    reservation: &WebsiteOperationReservation,
    previous_site: Option<&StoredWebsiteSnapshot>,
) -> Result<(), StoreError> {
    let snapshot = serde_json::to_string(&reservation.site)?;
    let previous_snapshot = previous_site.map(serde_json::to_string).transpose()?;
    tx.execute(
        "INSERT INTO website_operations (idempotency_key, fingerprint, operation, site_id, request_id, site_snapshot, job_id, previous_site_snapshot) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            key,
            fingerprint,
            reservation.operation.as_str(),
            reservation.site.id.to_string(),
            reservation.request_id.to_string(),
            snapshot,
            reservation.job_id,
            previous_snapshot,
        ],
    )?;
    Ok(())
}

impl ControlStore {
    /// Reserve a create operation before exposing its job to a worker.
    ///
    /// The idempotency record and desired site state are written in one SQLite
    /// transaction so a worker cannot claim an operation before the site row
    /// exists. A failed site with the same spec can be retried with a new key.
    pub fn reserve_website_create(
        &mut self,
        idempotency_key: &str,
        fingerprint: &Value,
        tenant_id: Uuid,
        proposed_site_id: Uuid,
        proposed_request_id: Uuid,
        domain: &str,
        spec: WebsiteSpec,
        now: &str,
    ) -> Result<WebsiteOperationReservation, StoreError> {
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
        if let Some(existing) =
            existing_reservation(&tx, key, &fingerprint, WebsiteOperation::Apply)?
        {
            tx.commit()?;
            return Ok(existing);
        }

        let (site, previous_site) = if let Some(previous) = get_stored_by_domain(&tx, domain)? {
            let mut existing = previous.record.clone();
            if existing.tenant_id != tenant_id
                || existing.state != WebsiteState::Failed
                || existing.spec != spec
            {
                return Err(StoreError::WebsiteConflict);
            }
            existing.state = WebsiteState::Provisioning;
            existing.last_error = None;
            existing.updated_at = now.to_string();
            tx.execute(
                "UPDATE websites SET state = ?1, last_error = NULL, updated_at = ?2, operation_request_id = ?3, job_id = NULL WHERE site_id = ?4",
                params![
                    WebsiteState::Provisioning.as_str(),
                    now,
                    proposed_request_id.to_string(),
                    existing.id.to_string(),
                ],
            )?;
            (existing, Some(previous))
        } else {
            let site = WebsiteRecord {
                id: proposed_site_id,
                tenant_id,
                domain: domain.to_string(),
                spec,
                state: WebsiteState::Provisioning,
                last_error: None,
                created_at: now.to_string(),
                updated_at: now.to_string(),
            };
            let spec_json = serde_json::to_string(&site.spec)?;
            tx.execute(
                "INSERT INTO websites (site_id, domain, spec, state, last_error, created_at, updated_at, operation_request_id, job_id, tenant_id) VALUES (?1, ?2, ?3, ?4, NULL, ?5, ?5, ?6, NULL, ?7)",
                params![
                    site.id.to_string(),
                    site.domain,
                    spec_json,
                    site.state.as_str(),
                    now,
                    proposed_request_id.to_string(),
                    tenant_id.to_string(),
                ],
            )?;
            (site, None)
        };

        let reservation = WebsiteOperationReservation {
            operation: WebsiteOperation::Apply,
            request_id: proposed_request_id,
            site,
            job_id: None,
            created: true,
        };
        save_reservation(&tx, key, &fingerprint, &reservation, previous_site.as_ref())?;
        tx.commit()?;
        Ok(reservation)
    }

    /// Reserve a delete operation. The site record remains until a successful
    /// agent completion is persisted.
    pub fn reserve_website_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &Value,
        tenant_id: Uuid,
        site_id: Uuid,
        request_id: Uuid,
        now: &str,
    ) -> Result<WebsiteOperationReservation, StoreError> {
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
        if let Some(existing) =
            existing_reservation(&tx, key, &fingerprint, WebsiteOperation::Delete)?
        {
            tx.commit()?;
            return Ok(existing);
        }

        let previous_site =
            get_stored_by_id(&tx, site_id)?.ok_or(StoreError::WebsiteNotFound(site_id))?;
        if previous_site.record.tenant_id != tenant_id {
            return Err(StoreError::WebsiteNotFound(site_id));
        }
        let mut site = previous_site.record.clone();
        if site.state == WebsiteState::Provisioning {
            return Err(StoreError::WebsiteOperationInProgress);
        }
        if site.state == WebsiteState::Deleting {
            return Err(StoreError::WebsiteConflict);
        }
        site.state = WebsiteState::Deleting;
        site.last_error = None;
        site.updated_at = now.to_string();
        tx.execute(
            "UPDATE websites SET state = ?1, last_error = NULL, updated_at = ?2, operation_request_id = ?3, job_id = NULL WHERE site_id = ?4",
            params![
                WebsiteState::Deleting.as_str(),
                now,
                request_id.to_string(),
                site_id.to_string(),
            ],
        )?;

        let reservation = WebsiteOperationReservation {
            operation: WebsiteOperation::Delete,
            request_id,
            site,
            job_id: None,
            created: true,
        };
        save_reservation(&tx, key, &fingerprint, &reservation, Some(&previous_site))?;
        tx.commit()?;
        Ok(reservation)
    }

    pub fn link_website_operation_job(
        &mut self,
        idempotency_key: &str,
        job_id: &str,
    ) -> Result<(), StoreError> {
        let key = idempotency_key.trim();
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let existing = tx
            .query_row(
                "SELECT site_id, request_id, job_id FROM website_operations WHERE idempotency_key = ?1",
                [key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?
            .ok_or(StoreError::IdempotencyConflict)?;
        if existing
            .2
            .as_deref()
            .is_some_and(|current| current != job_id)
        {
            return Err(StoreError::IdempotencyConflict);
        }
        tx.execute(
            "UPDATE website_operations SET job_id = ?1 WHERE idempotency_key = ?2",
            params![job_id, key],
        )?;
        tx.execute(
            "UPDATE websites SET job_id = ?1 WHERE site_id = ?2 AND operation_request_id = ?3",
            params![job_id, existing.0, existing.1],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Undo a reservation if the job queue rejected the operation. Existing
    /// records are restored to their exact prior state.
    pub fn rollback_website_operation(&mut self, idempotency_key: &str) -> Result<(), StoreError> {
        let key = idempotency_key.trim();
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let operation = tx
            .query_row(
                "SELECT site_id, request_id, previous_site_snapshot FROM website_operations WHERE idempotency_key = ?1",
                [key],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()?;
        let Some((site_id, request_id, previous_snapshot)) = operation else {
            tx.commit()?;
            return Ok(());
        };

        let current_request_id = tx
            .query_row(
                "SELECT operation_request_id FROM websites WHERE site_id = ?1",
                [&site_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        if current_request_id.as_deref() == Some(request_id.as_str()) {
            if let Some(previous_snapshot) = previous_snapshot {
                let previous: StoredWebsiteSnapshot = serde_json::from_str(&previous_snapshot)?;
                let spec = serde_json::to_string(&previous.record.spec)?;
                tx.execute(
                    "UPDATE websites SET domain = ?1, spec = ?2, state = ?3, last_error = ?4, created_at = ?5, updated_at = ?6, operation_request_id = ?7, job_id = ?8 WHERE site_id = ?9",
                    params![
                        previous.record.domain,
                        spec,
                        previous.record.state.as_str(),
                        previous.record.last_error,
                        previous.record.created_at,
                        previous.record.updated_at,
                        previous.request_id,
                        previous.job_id,
                        site_id,
                    ],
                )?;
            } else {
                tx.execute("DELETE FROM websites WHERE site_id = ?1", [&site_id])?;
            }
        }
        tx.execute(
            "DELETE FROM website_operations WHERE idempotency_key = ?1",
            [key],
        )?;
        tx.commit()?;
        Ok(())
    }

    /// Apply the persisted terminal job outcome only if it still belongs to
    /// the site's latest operation. Stale completions cannot overwrite a retry.
    pub fn complete_website_operation(
        &mut self,
        site_id: Uuid,
        request_id: Uuid,
        operation: WebsiteOperation,
        succeeded: bool,
        safe_error: Option<&str>,
        now: &str,
    ) -> Result<bool, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let current_request_id: Option<String> = tx
            .query_row(
                "SELECT operation_request_id FROM websites WHERE site_id = ?1",
                [site_id.to_string()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(current_request_id) = current_request_id else {
            tx.commit()?;
            return Ok(false);
        };
        if current_request_id != request_id.to_string() {
            tx.commit()?;
            return Ok(false);
        }

        match (operation, succeeded) {
            (WebsiteOperation::Apply, true) => {
                tx.execute(
                    "UPDATE websites SET state = ?1, last_error = NULL, updated_at = ?2 WHERE site_id = ?3 AND operation_request_id = ?4",
                    params![WebsiteState::Active.as_str(), now, site_id.to_string(), request_id.to_string()],
                )?;
            }
            (WebsiteOperation::Apply, false) | (WebsiteOperation::Delete, false) => {
                tx.execute(
                    "UPDATE websites SET state = ?1, last_error = ?2, updated_at = ?3 WHERE site_id = ?4 AND operation_request_id = ?5",
                    params![WebsiteState::Failed.as_str(), safe_error, now, site_id.to_string(), request_id.to_string()],
                )?;
            }
            (WebsiteOperation::Delete, true) => {
                tx.execute(
                    "DELETE FROM websites WHERE site_id = ?1 AND operation_request_id = ?2",
                    params![site_id.to_string(), request_id.to_string()],
                )?;
            }
        }
        tx.commit()?;
        Ok(true)
    }

    pub fn get_website(&self, site_id: Uuid) -> Result<WebsiteRecord, StoreError> {
        get_by_id(&self.conn, site_id)?.ok_or(StoreError::WebsiteNotFound(site_id))
    }

    pub fn get_website_for_tenant(
        &self,
        site_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<WebsiteRecord, StoreError> {
        get_by_id_for_tenant(&self.conn, site_id, tenant_id)?
            .ok_or(StoreError::WebsiteNotFound(site_id))
    }

    pub fn list_websites(&self) -> Result<Vec<WebsiteRecord>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT site_id, domain, spec, state, last_error, created_at, updated_at, operation_request_id, job_id, tenant_id FROM websites ORDER BY domain COLLATE NOCASE, site_id",
        )?;
        let rows = statement.query_map([], raw_website)?;
        rows.map(|row| decode_website(row?)).collect()
    }

    pub fn list_websites_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<WebsiteRecord>, StoreError> {
        let mut statement = self.conn.prepare(
            "SELECT site_id, domain, spec, state, last_error, created_at, updated_at, operation_request_id, job_id, tenant_id FROM websites WHERE tenant_id = ?1 ORDER BY domain COLLATE NOCASE, site_id",
        )?;
        let rows = statement.query_map([tenant_id.to_string()], raw_website)?;
        rows.map(|row| decode_website(row?)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn static_spec() -> WebsiteSpec {
        WebsiteSpec::Static {
            document_root: "/srv/fusionpanel/sites/app.local".to_string(),
        }
    }

    fn test_tenant_id() -> Uuid {
        Uuid::from_u128(1)
    }

    fn create_reservation(
        store: &mut ControlStore,
        key: &str,
        request_id: Uuid,
    ) -> WebsiteOperationReservation {
        let fingerprint = json!({
            "operation": "site.apply",
            "payload": {
                "domain": "app.local",
                "spec": static_spec(),
            },
        });
        store
            .reserve_website_create(
                key,
                &fingerprint,
                test_tenant_id(),
                Uuid::new_v4(),
                request_id,
                "app.local",
                static_spec(),
                "2026-11-16T00:00:00Z",
            )
            .unwrap()
    }

    #[test]
    fn website_create_reservation_is_idempotent_and_persistent() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let request_id = Uuid::new_v4();
        let original = create_reservation(&mut store, "create-app", request_id);
        assert_eq!(original.site.state, WebsiteState::Provisioning);
        assert_eq!(original.operation, WebsiteOperation::Apply);

        let replay = create_reservation(&mut store, "create-app", Uuid::new_v4());
        assert_eq!(replay.site, original.site);
        assert_eq!(replay.request_id, original.request_id);

        let conflict = json!({
            "operation": "site.apply",
            "payload": {
                "domain": "other.local",
                "spec": static_spec(),
            },
        });
        assert!(matches!(
            store.reserve_website_create(
                "create-app",
                &conflict,
                test_tenant_id(),
                Uuid::new_v4(),
                Uuid::new_v4(),
                "other.local",
                static_spec(),
                "2026-11-16T00:01:00Z",
            ),
            Err(StoreError::IdempotencyConflict)
        ));
        assert_eq!(store.list_websites().unwrap(), vec![original.site]);
    }

    #[test]
    fn failed_create_can_be_retried_and_stale_results_do_not_overwrite_it() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let first = create_reservation(&mut store, "create-app-1", Uuid::new_v4());
        store
            .complete_website_operation(
                first.site.id,
                first.request_id,
                WebsiteOperation::Apply,
                false,
                Some("Website configuration could not be applied"),
                "2026-11-16T00:01:00Z",
            )
            .unwrap();
        assert_eq!(
            store.get_website(first.site.id).unwrap().state,
            WebsiteState::Failed
        );

        let retry = create_reservation(&mut store, "create-app-2", Uuid::new_v4());
        assert_eq!(retry.site.id, first.site.id);
        assert_eq!(retry.site.state, WebsiteState::Provisioning);

        let changed = store
            .complete_website_operation(
                first.site.id,
                first.request_id,
                WebsiteOperation::Apply,
                true,
                None,
                "2026-11-16T00:02:00Z",
            )
            .unwrap();
        assert!(!changed);
        assert_eq!(
            store.get_website(first.site.id).unwrap().state,
            WebsiteState::Provisioning
        );
    }

    #[test]
    fn delete_preserves_record_until_success_and_replays_after_removal() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let created = create_reservation(&mut store, "create-app", Uuid::new_v4());
        store
            .complete_website_operation(
                created.site.id,
                created.request_id,
                WebsiteOperation::Apply,
                true,
                None,
                "2026-11-16T00:01:00Z",
            )
            .unwrap();

        let key = "delete-app";
        let request_id = Uuid::new_v4();
        let fingerprint = json!({
            "operation": "site.delete",
            "payload": {
                "site_id": created.site.id,
                "domain": "app.local",
            },
        });
        let deleting = store
            .reserve_website_delete(
                key,
                &fingerprint,
                test_tenant_id(),
                created.site.id,
                request_id,
                "2026-11-16T00:02:00Z",
            )
            .unwrap();
        assert_eq!(
            store.get_website(created.site.id).unwrap().state,
            WebsiteState::Deleting
        );

        store
            .complete_website_operation(
                created.site.id,
                request_id,
                WebsiteOperation::Delete,
                true,
                None,
                "2026-11-16T00:03:00Z",
            )
            .unwrap();
        assert!(matches!(
            store.get_website(created.site.id),
            Err(StoreError::WebsiteNotFound(_))
        ));

        let replay = store
            .reserve_website_delete(
                key,
                &fingerprint,
                test_tenant_id(),
                created.site.id,
                Uuid::new_v4(),
                "2026-11-16T00:04:00Z",
            )
            .unwrap();
        assert_eq!(replay.site, deleting.site);
        assert_eq!(replay.request_id, deleting.request_id);
    }

    #[test]
    fn delete_waits_for_apply_completion_before_reserving_delete() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let applying = create_reservation(&mut store, "create-app", Uuid::new_v4());
        let delete_fingerprint = json!({
            "operation": "site.delete",
            "payload": { "site_id": applying.site.id },
        });

        assert!(matches!(
            store.reserve_website_delete(
                "delete-app",
                &delete_fingerprint,
                test_tenant_id(),
                applying.site.id,
                Uuid::new_v4(),
                "2026-11-16T00:01:00Z",
            ),
            Err(StoreError::WebsiteOperationInProgress)
        ));
        assert_eq!(
            store.get_website(applying.site.id).unwrap().state,
            WebsiteState::Provisioning
        );

        assert!(store
            .complete_website_operation(
                applying.site.id,
                applying.request_id,
                WebsiteOperation::Apply,
                true,
                None,
                "2026-11-16T00:02:00Z",
            )
            .unwrap());

        let deleting = store
            .reserve_website_delete(
                "delete-app",
                &delete_fingerprint,
                test_tenant_id(),
                applying.site.id,
                Uuid::new_v4(),
                "2026-11-16T00:03:00Z",
            )
            .unwrap();
        assert_eq!(deleting.site.state, WebsiteState::Deleting);

        // A duplicate or delayed apply completion cannot overwrite the delete reservation.
        assert!(!store
            .complete_website_operation(
                applying.site.id,
                applying.request_id,
                WebsiteOperation::Apply,
                true,
                None,
                "2026-11-16T00:04:00Z",
            )
            .unwrap());
        assert_eq!(
            store.get_website(applying.site.id).unwrap().state,
            WebsiteState::Deleting
        );
    }

    #[test]
    fn website_records_and_idempotency_are_tenant_scoped() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let fingerprint = json!({
            "operation": "site.apply",
            "payload": {
                "domain": "tenant.example.test",
                "spec": static_spec(),
            },
        });
        let site = store
            .reserve_website_create(
                "tenant-create",
                &fingerprint,
                tenant_a,
                Uuid::new_v4(),
                Uuid::new_v4(),
                "tenant.example.test",
                static_spec(),
                "2026-11-16T00:00:00Z",
            )
            .unwrap();

        assert_eq!(
            store.list_websites_for_tenant(tenant_a).unwrap(),
            vec![site.site.clone()]
        );
        assert!(store.list_websites_for_tenant(tenant_b).unwrap().is_empty());
        assert_eq!(
            store
                .get_website_for_tenant(site.site.id, tenant_a)
                .unwrap(),
            site.site
        );
        assert!(matches!(
            store.get_website_for_tenant(site.site.id, tenant_b),
            Err(StoreError::WebsiteNotFound(_))
        ));
        assert!(matches!(
            store.reserve_website_delete(
                "foreign-delete",
                &json!({"operation": "site.delete", "site_id": site.site.id}),
                tenant_b,
                site.site.id,
                Uuid::new_v4(),
                "2026-11-16T00:01:00Z",
            ),
            Err(StoreError::WebsiteNotFound(_))
        ));
        assert!(matches!(
            store.reserve_website_create(
                "tenant-create",
                &fingerprint,
                tenant_b,
                Uuid::new_v4(),
                Uuid::new_v4(),
                "tenant.example.test",
                static_spec(),
                "2026-11-16T00:01:00Z",
            ),
            Err(StoreError::IdempotencyConflict)
        ));
    }
}

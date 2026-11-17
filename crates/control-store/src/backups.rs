#![forbid(unsafe_code)]

use crate::{ControlStore, StoreError};
use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BackupState {
    Creating,
    Ready,
    Restoring,
    Failed,
}

impl BackupState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Creating => "creating",
            Self::Ready => "ready",
            Self::Restoring => "restoring",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "creating" => Ok(Self::Creating),
            "ready" => Ok(Self::Ready),
            "restoring" => Ok(Self::Restoring),
            "failed" => Ok(Self::Failed),
            other => Err(StoreError::Parse(format!("unknown backup state '{other}'"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackupRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub archive_key: String,
    pub target: Value,
    pub destination_kind: String,
    pub destination_summary: String,
    pub state: BackupState,
    pub manifest_sha256: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BackupOperation {
    #[serde(rename = "backup.create")]
    Create,
    #[serde(rename = "backup.restore")]
    Restore,
}

impl BackupOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Create => "backup.create",
            Self::Restore => "backup.restore",
        }
    }
}

#[derive(Debug, Clone)]
pub struct BackupOperationReservation {
    pub backup: BackupRecord,
    pub request_id: Uuid,
    pub existing_job_id: Option<Uuid>,
    pub already_existed: bool,
}

impl ControlStore {
    pub fn list_backups_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<BackupRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT backup_id, tenant_id, archive_key, target_json, destination_kind, destination_summary, state, manifest_sha256, last_error, created_at, updated_at
             FROM backups
             WHERE tenant_id = ?1
             ORDER BY created_at DESC, backup_id ASC",
        )?;
        let rows = stmt.query_map([tenant_id.to_string()], backup_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_backup_for_tenant(
        &self,
        backup_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<BackupRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT backup_id, tenant_id, archive_key, target_json, destination_kind, destination_summary, state, manifest_sha256, last_error, created_at, updated_at
                 FROM backups
                 WHERE backup_id = ?1 AND tenant_id = ?2",
                params![backup_id.to_string(), tenant_id.to_string()],
                backup_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_backup_create(
        &mut self,
        tenant_id: Uuid,
        backup_id: Uuid,
        archive_key: &str,
        target: Value,
        destination_kind: &str,
        destination_summary: &str,
        idempotency_key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<BackupOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(existing) = load_backup_operation(&tx, idempotency_key, fingerprint)? {
            tx.commit()?;
            return Ok(existing);
        }

        let request_id = Uuid::new_v4();
        let record = BackupRecord {
            id: backup_id,
            tenant_id,
            archive_key: archive_key.to_string(),
            target: target.clone(),
            destination_kind: destination_kind.to_string(),
            destination_summary: destination_summary.to_string(),
            state: BackupState::Creating,
            manifest_sha256: None,
            last_error: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        };
        let target_json = serde_json::to_string(&target)?;
        tx.execute(
            "INSERT INTO backups (backup_id, tenant_id, archive_key, target_json, destination_kind, destination_summary, state, manifest_sha256, last_error, created_at, updated_at, operation_request_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'creating', NULL, NULL, ?7, ?8, ?9)",
            params![
                record.id.to_string(),
                record.tenant_id.to_string(),
                record.archive_key,
                target_json,
                record.destination_kind,
                record.destination_summary,
                record.created_at,
                record.updated_at,
                request_id.to_string(),
            ],
        )?;
        let snapshot = serde_json::to_string(&record)?;
        tx.execute(
            "INSERT INTO backup_operations (idempotency_key, fingerprint, operation, backup_id, request_id, snapshot, job_id)
             VALUES (?1, ?2, 'backup.create', ?3, ?4, ?5, NULL)",
            params![
                idempotency_key,
                fingerprint,
                record.id.to_string(),
                request_id.to_string(),
                snapshot,
            ],
        )?;
        tx.commit()?;

        Ok(BackupOperationReservation {
            backup: record,
            request_id,
            existing_job_id: None,
            already_existed: false,
        })
    }

    pub fn reserve_backup_restore(
        &mut self,
        backup_id: Uuid,
        tenant_id: Uuid,
        idempotency_key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<Option<BackupOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(existing) = load_backup_operation(&tx, idempotency_key, fingerprint)? {
            if existing.backup.tenant_id != tenant_id {
                return Ok(None);
            }
            tx.commit()?;
            return Ok(Some(existing));
        }

        let current = tx
            .query_row(
                "SELECT backup_id, tenant_id, archive_key, target_json, destination_kind, destination_summary, state, manifest_sha256, last_error, created_at, updated_at
                 FROM backups
                 WHERE backup_id = ?1 AND tenant_id = ?2",
                params![backup_id.to_string(), tenant_id.to_string()],
                backup_from_row,
            )
            .optional()?;
        let Some(mut record) = current else {
            return Ok(None);
        };

        if record.state != BackupState::Ready {
            return Err(StoreError::DatabaseOperationInProgress);
        }

        let request_id = Uuid::new_v4();
        record.state = BackupState::Restoring;
        record.last_error = None;
        record.updated_at = now.to_string();

        tx.execute(
            "UPDATE backups SET state = 'restoring', last_error = NULL, updated_at = ?1, operation_request_id = ?2
             WHERE backup_id = ?3",
            params![now, request_id.to_string(), record.id.to_string()],
        )?;
        let snapshot = serde_json::to_string(&record)?;
        tx.execute(
            "INSERT INTO backup_operations (idempotency_key, fingerprint, operation, backup_id, request_id, snapshot, job_id)
             VALUES (?1, ?2, 'backup.restore', ?3, ?4, ?5, NULL)",
            params![
                idempotency_key,
                fingerprint,
                record.id.to_string(),
                request_id.to_string(),
                snapshot,
            ],
        )?;
        tx.commit()?;

        Ok(Some(BackupOperationReservation {
            backup: record,
            request_id,
            existing_job_id: None,
            already_existed: false,
        }))
    }

    pub fn attach_backup_operation_job(
        &mut self,
        idempotency_key: &str,
        job_id: Uuid,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE backup_operations SET job_id = COALESCE(job_id, ?1) WHERE idempotency_key = ?2",
            params![job_id.to_string(), idempotency_key],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn complete_backup_operation(
        &mut self,
        operation: BackupOperation,
        backup_id: Uuid,
        request_id: Uuid,
        succeeded: bool,
        manifest_sha256: Option<&str>,
        error_text: Option<&str>,
        now: &str,
    ) -> Result<(), StoreError> {
        let next_state = if succeeded { "ready" } else { "failed" };
        let next_error = if succeeded { None } else { error_text };
        match operation {
            BackupOperation::Create => {
                self.conn.execute(
                    "UPDATE backups SET state = ?1, manifest_sha256 = COALESCE(?2, manifest_sha256), last_error = ?3, updated_at = ?4
                     WHERE backup_id = ?5 AND operation_request_id = ?6",
                    params![
                        next_state,
                        manifest_sha256,
                        next_error,
                        now,
                        backup_id.to_string(),
                        request_id.to_string()
                    ],
                )?;
            }
            BackupOperation::Restore => {
                self.conn.execute(
                    "UPDATE backups SET state = ?1, last_error = ?2, updated_at = ?3
                     WHERE backup_id = ?4 AND operation_request_id = ?5",
                    params![
                        next_state,
                        next_error,
                        now,
                        backup_id.to_string(),
                        request_id.to_string()
                    ],
                )?;
            }
        }
        Ok(())
    }
}

fn load_backup_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<BackupOperationReservation>, StoreError> {
    let existing = tx
        .query_row(
            "SELECT fingerprint, request_id, snapshot, job_id
             FROM backup_operations WHERE idempotency_key = ?1",
            [idempotency_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                ))
            },
        )
        .optional()?;
    let Some((saved_fp, req_raw, snapshot_raw, job_raw)) = existing else {
        return Ok(None);
    };
    if saved_fp != fingerprint {
        return Err(StoreError::IdempotencyConflict);
    }
    let backup: BackupRecord = serde_json::from_str(&snapshot_raw)?;
    let request_id = Uuid::parse_str(&req_raw)
        .map_err(|e| StoreError::Parse(format!("invalid request_id UUID: {e}")))?;
    let existing_job_id = job_raw
        .map(|raw| Uuid::parse_str(&raw))
        .transpose()
        .map_err(|e| StoreError::Parse(format!("invalid job_id UUID: {e}")))?;
    Ok(Some(BackupOperationReservation {
        backup,
        request_id,
        existing_job_id,
        already_existed: true,
    }))
}

fn backup_from_row(row: &Row<'_>) -> rusqlite::Result<BackupRecord> {
    let id_raw: String = row.get(0)?;
    let tenant_raw: String = row.get(1)?;
    let target_raw: String = row.get(3)?;
    let state_raw: String = row.get(6)?;

    let id = Uuid::parse_str(&id_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tenant_id = Uuid::parse_str(&tenant_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let target: Value = serde_json::from_str(&target_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let state = BackupState::parse(&state_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(BackupRecord {
        id,
        tenant_id,
        archive_key: row.get(2)?,
        target,
        destination_kind: row.get(4)?,
        destination_summary: row.get(5)?,
        state,
        manifest_sha256: row.get(7)?,
        last_error: row.get(8)?,
        created_at: row.get(9)?,
        updated_at: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn backup_create_and_restore_lifecycle_is_idempotent_and_tenant_scoped() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();
        let backup_id = Uuid::new_v4();

        let created = store
            .reserve_backup_create(
                tenant_a,
                backup_id,
                "tenant/backup-1.tar.gz",
                json!({"kind":"site"}),
                "local",
                "/var/backups/fusionpanel",
                "bk-1",
                "fp-1",
                "2026-11-17T12:00:00Z",
            )
            .unwrap();
        assert!(!created.already_existed);
        assert_eq!(created.backup.state, BackupState::Creating);

        let sha = "0".repeat(64);
        store
            .complete_backup_operation(
                BackupOperation::Create,
                backup_id,
                created.request_id,
                true,
                Some(&sha),
                None,
                "2026-11-17T12:00:01Z",
            )
            .unwrap();

        let fetched = store
            .get_backup_for_tenant(backup_id, tenant_a)
            .unwrap()
            .unwrap();
        assert_eq!(fetched.state, BackupState::Ready);
        assert_eq!(fetched.manifest_sha256.as_deref(), Some(sha.as_str()));
        assert!(store
            .get_backup_for_tenant(backup_id, tenant_b)
            .unwrap()
            .is_none());

        let restored = store
            .reserve_backup_restore(
                backup_id,
                tenant_a,
                "bk-restore-1",
                "fp-restore-1",
                "2026-11-17T12:00:02Z",
            )
            .unwrap()
            .unwrap();
        assert_eq!(restored.backup.state, BackupState::Restoring);

        store
            .complete_backup_operation(
                BackupOperation::Restore,
                backup_id,
                restored.request_id,
                true,
                None,
                None,
                "2026-11-17T12:00:03Z",
            )
            .unwrap();
        assert_eq!(
            store
                .get_backup_for_tenant(backup_id, tenant_a)
                .unwrap()
                .unwrap()
                .state,
            BackupState::Ready
        );
    }
}

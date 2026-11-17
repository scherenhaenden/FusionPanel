#![forbid(unsafe_code)]

use crate::{ControlStore, StoreError};
use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CronJobState {
    Provisioning,
    Ready,
    Failed,
    Deleting,
}

impl CronJobState {
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
            other => Err(StoreError::Parse(format!("unknown cron state '{other}'"))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CronJobRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub schedule: String,
    pub command: String,
    pub enabled: bool,
    pub state: CronJobState,
    pub last_run_at: Option<String>,
    pub last_run_status: Option<String>,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum CronOperation {
    Apply,
    Delete,
    RunNow,
}

impl CronOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "cron.apply",
            Self::Delete => "cron.delete",
            Self::RunNow => "cron.run_now",
        }
    }
}

#[derive(Debug, Clone)]
pub struct CronOperationReservation {
    pub cron_job: CronJobRecord,
    pub tenant_jobs: Vec<CronJobRecord>,
    pub request_id: Uuid,
    pub job_id: Option<Uuid>,
    pub idempotent_replay: bool,
}

impl ControlStore {
    pub fn list_cron_jobs(&mut self, tenant_id: Uuid) -> Result<Vec<CronJobRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT cron_id, tenant_id, name, schedule, command, enabled, state, last_run_at, last_run_status, last_error, created_at, updated_at
             FROM cron_jobs WHERE tenant_id = ?1 ORDER BY created_at DESC, cron_id DESC",
        )?;
        let rows = stmt.query_map([tenant_id.to_string()], row_to_cron_job)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_cron_job(&mut self, cron_id: Uuid) -> Result<Option<CronJobRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT cron_id, tenant_id, name, schedule, command, enabled, state, last_run_at, last_run_status, last_error, created_at, updated_at
                 FROM cron_jobs WHERE cron_id = ?1",
                [cron_id.to_string()],
                row_to_cron_job,
            )
            .optional()
            .map_err(Into::into)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_cron_create(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        tenant_id: Uuid,
        name: &str,
        schedule: &str,
        command: &str,
        enabled: bool,
        now: &str,
    ) -> Result<CronOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_cron_operation(&tx, idempotency_key, fingerprint, CronOperation::Apply)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let cron_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let record = CronJobRecord {
            id: cron_id,
            tenant_id,
            name: name.to_string(),
            schedule: schedule.to_string(),
            command: command.to_string(),
            enabled,
            state: CronJobState::Provisioning,
            last_run_at: None,
            last_run_status: None,
            last_error: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        };
        tx.execute(
            "INSERT INTO cron_jobs (cron_id, tenant_id, name, schedule, command, enabled, state, last_run_at, last_run_status, last_error, created_at, updated_at, operation_request_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'provisioning', NULL, NULL, NULL, ?7, ?8, ?9)",
            params![
                record.id.to_string(),
                record.tenant_id.to_string(),
                record.name,
                record.schedule,
                record.command,
                if record.enabled { 1i64 } else { 0i64 },
                record.created_at,
                record.updated_at,
                request_id.to_string(),
            ],
        )?;

        let tenant_jobs = load_tenant_cron_jobs_tx(&tx, tenant_id)?;
        insert_cron_operation(
            &tx,
            idempotency_key,
            fingerprint,
            CronOperation::Apply,
            cron_id,
            request_id,
            &record,
            &tenant_jobs,
        )?;
        tx.commit()?;

        Ok(CronOperationReservation {
            cron_job: record,
            tenant_jobs,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_cron_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        cron_id: Uuid,
        now: &str,
    ) -> Result<CronOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_cron_operation(&tx, idempotency_key, fingerprint, CronOperation::Delete)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let mut record = load_cron_job_tx(&tx, cron_id)?;
        let request_id = Uuid::new_v4();
        record.state = CronJobState::Deleting;
        record.last_error = None;
        record.updated_at = now.to_string();
        tx.execute(
            "UPDATE cron_jobs SET state = 'deleting', last_error = NULL, updated_at = ?1, operation_request_id = ?2 WHERE cron_id = ?3",
            params![record.updated_at, request_id.to_string(), cron_id.to_string()],
        )?;

        let mut tenant_jobs = load_tenant_cron_jobs_tx(&tx, record.tenant_id)?;
        tenant_jobs.retain(|j| j.id != cron_id);
        insert_cron_operation(
            &tx,
            idempotency_key,
            fingerprint,
            CronOperation::Delete,
            cron_id,
            request_id,
            &record,
            &tenant_jobs,
        )?;
        tx.commit()?;

        Ok(CronOperationReservation {
            cron_job: record,
            tenant_jobs,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_cron_run_now(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        cron_id: Uuid,
    ) -> Result<CronOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_cron_operation(&tx, idempotency_key, fingerprint, CronOperation::RunNow)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let record = load_cron_job_tx(&tx, cron_id)?;
        let request_id = Uuid::new_v4();
        tx.execute(
            "UPDATE cron_jobs SET operation_request_id = ?1 WHERE cron_id = ?2",
            params![request_id.to_string(), cron_id.to_string()],
        )?;
        let tenant_jobs = load_tenant_cron_jobs_tx(&tx, record.tenant_id)?;
        insert_cron_operation(
            &tx,
            idempotency_key,
            fingerprint,
            CronOperation::RunNow,
            cron_id,
            request_id,
            &record,
            &tenant_jobs,
        )?;
        tx.commit()?;

        Ok(CronOperationReservation {
            cron_job: record,
            tenant_jobs,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn set_cron_operation_job_id(
        &mut self,
        idempotency_key: &str,
        job_id: Uuid,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE cron_operations SET job_id = ?1 WHERE idempotency_key = ?2",
            params![job_id.to_string(), idempotency_key],
        )?;
        Ok(())
    }

    pub fn complete_cron_operation(
        &mut self,
        request_id: Uuid,
        operation: CronOperation,
        succeeded: bool,
        message: Option<&str>,
        now: &str,
    ) -> Result<bool, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let cron_id_str: Option<String> = tx
            .query_row(
                "SELECT cron_id FROM cron_operations WHERE request_id = ?1 AND operation = ?2",
                params![request_id.to_string(), operation.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(cron_id_str) = cron_id_str else {
            return Ok(false);
        };

        let active_req: Option<String> = tx
            .query_row(
                "SELECT operation_request_id FROM cron_jobs WHERE cron_id = ?1",
                [&cron_id_str],
                |row| row.get(0),
            )
            .optional()?;
        if active_req.as_deref() != Some(&request_id.to_string()) {
            tx.commit()?;
            return Ok(true);
        }

        match operation {
            CronOperation::Apply => {
                if succeeded {
                    tx.execute(
                        "UPDATE cron_jobs SET state = 'ready', last_error = NULL, updated_at = ?1 WHERE cron_id = ?2",
                        params![now, cron_id_str],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE cron_jobs SET state = 'failed', last_error = ?1, updated_at = ?2 WHERE cron_id = ?3",
                        params![message.unwrap_or("cron.apply failed"), now, cron_id_str],
                    )?;
                }
            }
            CronOperation::Delete => {
                if succeeded {
                    tx.execute("DELETE FROM cron_jobs WHERE cron_id = ?1", [&cron_id_str])?;
                } else {
                    tx.execute(
                        "UPDATE cron_jobs SET state = 'failed', last_error = ?1, updated_at = ?2 WHERE cron_id = ?3",
                        params![message.unwrap_or("cron.delete failed"), now, cron_id_str],
                    )?;
                }
            }
            CronOperation::RunNow => {
                let status = if succeeded {
                    message.unwrap_or("succeeded")
                } else {
                    message.unwrap_or("failed")
                };
                tx.execute(
                    "UPDATE cron_jobs SET last_run_at = ?1, last_run_status = ?2, updated_at = ?3 WHERE cron_id = ?4",
                    params![now, status, now, cron_id_str],
                )?;
            }
        }

        tx.commit()?;
        Ok(true)
    }
}

#[derive(Serialize, Deserialize)]
struct CronOpSnapshot {
    cron_job: CronJobRecord,
    tenant_jobs: Vec<CronJobRecord>,
}

fn load_cron_job_tx(tx: &Transaction<'_>, cron_id: Uuid) -> Result<CronJobRecord, StoreError> {
    tx.query_row(
        "SELECT cron_id, tenant_id, name, schedule, command, enabled, state, last_run_at, last_run_status, last_error, created_at, updated_at
         FROM cron_jobs WHERE cron_id = ?1",
        [cron_id.to_string()],
        row_to_cron_job,
    )
    .optional()?
    .ok_or(StoreError::NotFound(cron_id))
}

fn load_tenant_cron_jobs_tx(
    tx: &Transaction<'_>,
    tenant_id: Uuid,
) -> Result<Vec<CronJobRecord>, StoreError> {
    let mut stmt = tx.prepare(
        "SELECT cron_id, tenant_id, name, schedule, command, enabled, state, last_run_at, last_run_status, last_error, created_at, updated_at
         FROM cron_jobs WHERE tenant_id = ?1 AND state != 'deleting' ORDER BY created_at ASC, cron_id ASC",
    )?;
    let rows = stmt.query_map([tenant_id.to_string()], row_to_cron_job)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

fn find_existing_cron_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    expected_op: CronOperation,
) -> Result<Option<CronOperationReservation>, StoreError> {
    let row: Option<(String, String, String, String, Option<String>)> = tx
        .query_row(
            "SELECT fingerprint, operation, request_id, snapshot, job_id FROM cron_operations WHERE idempotency_key = ?1",
            [idempotency_key],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)),
        )
        .optional()?;
    let Some((stored_fp, stored_op, req_id_str, snapshot_json, job_id_str)) = row else {
        return Ok(None);
    };
    if stored_fp != fingerprint || stored_op != expected_op.as_str() {
        return Err(StoreError::Conflict {
            expected: 0,
            actual: 1,
        });
    }
    let snapshot: CronOpSnapshot = serde_json::from_str(&snapshot_json)?;
    let request_id = Uuid::parse_str(&req_id_str)
        .map_err(|e| StoreError::Parse(format!("invalid cron request_id: {e}")))?;
    let job_id = job_id_str
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| StoreError::Parse(format!("invalid cron job_id: {e}")))?;
    Ok(Some(CronOperationReservation {
        cron_job: snapshot.cron_job,
        tenant_jobs: snapshot.tenant_jobs,
        request_id,
        job_id,
        idempotent_replay: true,
    }))
}

#[allow(clippy::too_many_arguments)]
fn insert_cron_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    operation: CronOperation,
    cron_id: Uuid,
    request_id: Uuid,
    cron_job: &CronJobRecord,
    tenant_jobs: &[CronJobRecord],
) -> Result<(), StoreError> {
    let snapshot_json = serde_json::to_string(&CronOpSnapshot {
        cron_job: cron_job.clone(),
        tenant_jobs: tenant_jobs.to_vec(),
    })?;
    tx.execute(
        "INSERT INTO cron_operations (idempotency_key, fingerprint, operation, cron_id, request_id, snapshot, job_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![
            idempotency_key,
            fingerprint,
            operation.as_str(),
            cron_id.to_string(),
            request_id.to_string(),
            snapshot_json,
        ],
    )?;
    Ok(())
}

fn row_to_cron_job(row: &Row<'_>) -> rusqlite::Result<CronJobRecord> {
    let id_str: String = row.get(0)?;
    let tenant_id_str: String = row.get(1)?;
    let name: String = row.get(2)?;
    let schedule: String = row.get(3)?;
    let command: String = row.get(4)?;
    let enabled: i64 = row.get(5)?;
    let state_str: String = row.get(6)?;
    let last_run_at: Option<String> = row.get(7)?;
    let last_run_status: Option<String> = row.get(8)?;
    let last_error: Option<String> = row.get(9)?;
    let created_at: String = row.get(10)?;
    let updated_at: String = row.get(11)?;

    let id = Uuid::parse_str(&id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tenant_id = Uuid::parse_str(&tenant_id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let state = CronJobState::parse(&state_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(6, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(CronJobRecord {
        id,
        tenant_id,
        name,
        schedule,
        command,
        enabled: enabled != 0,
        state,
        last_run_at,
        last_run_status,
        last_error,
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cron_create_run_and_delete_lifecycle() {
        let db_path = std::env::temp_dir().join(format!("fp-cron-test-{}.db", Uuid::new_v4()));
        let mut store = ControlStore::new(&db_path).unwrap();
        let tenant = Uuid::new_v4();
        let now = "2026-11-17T15:00:00Z";

        let created = store
            .reserve_cron_create(
                "k1",
                "fp1",
                tenant,
                "ping",
                "* * * * *",
                "echo ok",
                true,
                now,
            )
            .unwrap();
        assert_eq!(created.cron_job.state, CronJobState::Provisioning);
        assert!(store
            .complete_cron_operation(created.request_id, CronOperation::Apply, true, None, now)
            .unwrap());

        let run = store
            .reserve_cron_run_now("k2", "fp2", created.cron_job.id)
            .unwrap();
        assert!(store
            .complete_cron_operation(
                run.request_id,
                CronOperation::RunNow,
                true,
                Some("exit 0: ok"),
                now
            )
            .unwrap());

        let loaded = store.get_cron_job(created.cron_job.id).unwrap().unwrap();
        assert_eq!(loaded.state, CronJobState::Ready);
        assert_eq!(loaded.last_run_status.as_deref(), Some("exit 0: ok"));
        let _ = std::fs::remove_file(db_path);
    }
}

#![forbid(unsafe_code)]

use crate::{ControlStore, StoreError};
use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DnsZoneState {
    Provisioning,
    Ready,
    Failed,
    Deleting,
}

impl DnsZoneState {
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
                "unknown dns zone state '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsZoneRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub domain: String,
    pub serial: u32,
    pub default_ttl: u32,
    pub state: DnsZoneState,
    pub last_error: Option<String>,
    pub last_probe_result: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsRecordEntry {
    pub id: Uuid,
    pub zone_id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub record_type: String,
    pub value: String,
    pub ttl: u32,
    pub priority: Option<u16>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DnsZoneWithRecords {
    pub zone: DnsZoneRecord,
    pub records: Vec<DnsRecordEntry>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DnsOperation {
    ZoneApply,
    ZoneDelete,
    Probe,
}

impl DnsOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ZoneApply => "dns.zone.apply",
            Self::ZoneDelete => "dns.zone.delete",
            Self::Probe => "dns.probe",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "dns.zone.apply" => Ok(Self::ZoneApply),
            "dns.zone.delete" => Ok(Self::ZoneDelete),
            "dns.probe" => Ok(Self::Probe),
            other => Err(StoreError::Parse(format!(
                "unknown dns operation '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DnsOperationReservation {
    pub zone: DnsZoneRecord,
    pub records: Vec<DnsRecordEntry>,
    pub request_id: Uuid,
    pub job_id: Option<Uuid>,
    pub idempotent_replay: bool,
}

#[derive(Debug, Clone)]
pub struct DnsRecordMutation<'a> {
    pub name: &'a str,
    pub record_type: &'a str,
    pub value: &'a str,
    pub ttl: u32,
    pub priority: Option<u16>,
}

impl ControlStore {
    pub fn list_dns_zones(
        &mut self,
        tenant_id: Option<Uuid>,
    ) -> Result<Vec<DnsZoneRecord>, StoreError> {
        let mut zones = Vec::new();
        if let Some(tid) = tenant_id {
            let mut stmt = self.conn.prepare(
                "SELECT zone_id, tenant_id, domain, serial, default_ttl, state, last_error, last_probe_result, created_at, updated_at
                 FROM dns_zones WHERE tenant_id = ?1 ORDER BY created_at DESC, zone_id DESC",
            )?;
            let rows = stmt.query_map([tid.to_string()], row_to_zone)?;
            for row in rows {
                zones.push(row?);
            }
        } else {
            let mut stmt = self.conn.prepare(
                "SELECT zone_id, tenant_id, domain, serial, default_ttl, state, last_error, last_probe_result, created_at, updated_at
                 FROM dns_zones ORDER BY created_at DESC, zone_id DESC",
            )?;
            let rows = stmt.query_map([], row_to_zone)?;
            for row in rows {
                zones.push(row?);
            }
        }
        Ok(zones)
    }

    pub fn get_dns_zone(
        &mut self,
        zone_id: Uuid,
    ) -> Result<Option<DnsZoneWithRecords>, StoreError> {
        let zone = self
            .conn
            .query_row(
                "SELECT zone_id, tenant_id, domain, serial, default_ttl, state, last_error, last_probe_result, created_at, updated_at
                 FROM dns_zones WHERE zone_id = ?1",
                [zone_id.to_string()],
                row_to_zone,
            )
            .optional()?;
        let Some(zone) = zone else {
            return Ok(None);
        };
        let records = load_zone_records(&self.conn, zone_id)?;
        Ok(Some(DnsZoneWithRecords { zone, records }))
    }

    pub fn reserve_dns_zone_create(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        tenant_id: Uuid,
        domain: &str,
        default_ttl: u32,
        now: &str,
    ) -> Result<DnsOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

        if let Some(replay) =
            find_existing_dns_operation(&tx, idempotency_key, fingerprint, DnsOperation::ZoneApply)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let exists: Option<String> = tx
            .query_row(
                "SELECT zone_id FROM dns_zones WHERE domain = ?1",
                [domain],
                |row| row.get(0),
            )
            .optional()?;
        if exists.is_some() {
            return Err(StoreError::Conflict {
                expected: 0,
                actual: 1,
            });
        }

        let zone_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let zone = DnsZoneRecord {
            id: zone_id,
            tenant_id,
            domain: domain.to_string(),
            serial: 1,
            default_ttl,
            state: DnsZoneState::Provisioning,
            last_error: None,
            last_probe_result: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        };

        tx.execute(
            "INSERT INTO dns_zones (zone_id, tenant_id, domain, serial, default_ttl, state, last_error, last_probe_result, created_at, updated_at, operation_request_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL, ?7, ?8, ?9)",
            params![
                zone.id.to_string(),
                zone.tenant_id.to_string(),
                zone.domain,
                zone.serial,
                zone.default_ttl,
                zone.state.as_str(),
                zone.created_at,
                zone.updated_at,
                request_id.to_string(),
            ],
        )?;

        let snapshot = DnsZoneWithRecords {
            zone: zone.clone(),
            records: Vec::new(),
        };
        insert_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::ZoneApply,
            zone_id,
            request_id,
            &snapshot,
        )?;
        tx.commit()?;

        Ok(DnsOperationReservation {
            zone,
            records: Vec::new(),
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_dns_record_create(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        zone_id: Uuid,
        record: DnsRecordMutation<'_>,
        now: &str,
    ) -> Result<DnsOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_dns_operation(&tx, idempotency_key, fingerprint, DnsOperation::ZoneApply)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let mut zone = load_zone_tx(&tx, zone_id)?;
        let record_id = Uuid::new_v4();
        tx.execute(
            "INSERT INTO dns_records (record_id, zone_id, tenant_id, name, record_type, value, ttl, priority, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                record_id.to_string(),
                zone_id.to_string(),
                zone.tenant_id.to_string(),
                record.name,
                record.record_type,
                record.value,
                record.ttl,
                record.priority.map(i64::from),
                now,
                now,
            ],
        )?;

        let request_id = bump_zone_for_apply(&tx, &mut zone, now)?;
        let records = load_zone_records(&tx, zone_id)?;
        let snapshot = DnsZoneWithRecords {
            zone: zone.clone(),
            records: records.clone(),
        };
        insert_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::ZoneApply,
            zone_id,
            request_id,
            &snapshot,
        )?;
        tx.commit()?;

        Ok(DnsOperationReservation {
            zone,
            records,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_dns_record_update(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        zone_id: Uuid,
        record_id: Uuid,
        record: DnsRecordMutation<'_>,
        now: &str,
    ) -> Result<DnsOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_dns_operation(&tx, idempotency_key, fingerprint, DnsOperation::ZoneApply)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let mut zone = load_zone_tx(&tx, zone_id)?;
        let updated = tx.execute(
            "UPDATE dns_records
             SET name = ?1, record_type = ?2, value = ?3, ttl = ?4, priority = ?5, updated_at = ?6
             WHERE record_id = ?7 AND zone_id = ?8",
            params![
                record.name,
                record.record_type,
                record.value,
                record.ttl,
                record.priority.map(i64::from),
                now,
                record_id.to_string(),
                zone_id.to_string(),
            ],
        )?;
        if updated == 0 {
            return Err(StoreError::NotFound(record_id));
        }

        let request_id = bump_zone_for_apply(&tx, &mut zone, now)?;
        let records = load_zone_records(&tx, zone_id)?;
        let snapshot = DnsZoneWithRecords {
            zone: zone.clone(),
            records: records.clone(),
        };
        insert_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::ZoneApply,
            zone_id,
            request_id,
            &snapshot,
        )?;
        tx.commit()?;

        Ok(DnsOperationReservation {
            zone,
            records,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_dns_record_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        zone_id: Uuid,
        record_id: Uuid,
        now: &str,
    ) -> Result<DnsOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_dns_operation(&tx, idempotency_key, fingerprint, DnsOperation::ZoneApply)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let mut zone = load_zone_tx(&tx, zone_id)?;
        let deleted = tx.execute(
            "DELETE FROM dns_records WHERE record_id = ?1 AND zone_id = ?2",
            params![record_id.to_string(), zone_id.to_string()],
        )?;
        if deleted == 0 {
            return Err(StoreError::NotFound(record_id));
        }

        let request_id = bump_zone_for_apply(&tx, &mut zone, now)?;
        let records = load_zone_records(&tx, zone_id)?;
        let snapshot = DnsZoneWithRecords {
            zone: zone.clone(),
            records: records.clone(),
        };
        insert_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::ZoneApply,
            zone_id,
            request_id,
            &snapshot,
        )?;
        tx.commit()?;

        Ok(DnsOperationReservation {
            zone,
            records,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn reserve_dns_dkim_upsert(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        zone_id: Uuid,
        selector: &str,
        txt_value: &str,
        ttl: u32,
        now: &str,
    ) -> Result<DnsOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_dns_operation(&tx, idempotency_key, fingerprint, DnsOperation::ZoneApply)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let mut zone = load_zone_tx(&tx, zone_id)?;
        let owner_name = format!("{selector}._domainkey");
        let existing_id: Option<String> = tx
            .query_row(
                "SELECT record_id FROM dns_records WHERE zone_id = ?1 AND name = ?2 AND record_type = 'TXT'",
                params![zone_id.to_string(), owner_name],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(existing_id) = existing_id {
            tx.execute(
                "UPDATE dns_records SET value = ?1, ttl = ?2, updated_at = ?3 WHERE record_id = ?4",
                params![txt_value, ttl, now, existing_id],
            )?;
        } else {
            let record_id = Uuid::new_v4();
            tx.execute(
                "INSERT INTO dns_records (record_id, zone_id, tenant_id, name, record_type, value, ttl, priority, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 'TXT', ?5, ?6, NULL, ?7, ?8)",
                params![
                    record_id.to_string(),
                    zone_id.to_string(),
                    zone.tenant_id.to_string(),
                    owner_name,
                    txt_value,
                    ttl,
                    now,
                    now,
                ],
            )?;
        }

        let request_id = bump_zone_for_apply(&tx, &mut zone, now)?;
        let records = load_zone_records(&tx, zone_id)?;
        let snapshot = DnsZoneWithRecords {
            zone: zone.clone(),
            records: records.clone(),
        };
        insert_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::ZoneApply,
            zone_id,
            request_id,
            &snapshot,
        )?;
        tx.commit()?;

        Ok(DnsOperationReservation {
            zone,
            records,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_dns_zone_delete(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        zone_id: Uuid,
        now: &str,
    ) -> Result<DnsOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) = find_existing_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::ZoneDelete,
        )? {
            tx.commit()?;
            return Ok(replay);
        }

        let mut zone = load_zone_tx(&tx, zone_id)?;
        let request_id = Uuid::new_v4();
        zone.state = DnsZoneState::Deleting;
        zone.last_error = None;
        zone.updated_at = now.to_string();
        tx.execute(
            "UPDATE dns_zones SET state = 'deleting', last_error = NULL, updated_at = ?1, operation_request_id = ?2 WHERE zone_id = ?3",
            params![zone.updated_at, request_id.to_string(), zone_id.to_string()],
        )?;

        let records = load_zone_records(&tx, zone_id)?;
        let snapshot = DnsZoneWithRecords {
            zone: zone.clone(),
            records: records.clone(),
        };
        insert_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::ZoneDelete,
            zone_id,
            request_id,
            &snapshot,
        )?;
        tx.commit()?;

        Ok(DnsOperationReservation {
            zone,
            records,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn reserve_dns_probe(
        &mut self,
        idempotency_key: &str,
        fingerprint: &str,
        zone_id: Uuid,
    ) -> Result<DnsOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(replay) =
            find_existing_dns_operation(&tx, idempotency_key, fingerprint, DnsOperation::Probe)?
        {
            tx.commit()?;
            return Ok(replay);
        }

        let zone = load_zone_tx(&tx, zone_id)?;
        let request_id = Uuid::new_v4();
        tx.execute(
            "UPDATE dns_zones SET operation_request_id = ?1 WHERE zone_id = ?2",
            params![request_id.to_string(), zone_id.to_string()],
        )?;
        let records = load_zone_records(&tx, zone_id)?;
        let snapshot = DnsZoneWithRecords {
            zone: zone.clone(),
            records: records.clone(),
        };
        insert_dns_operation(
            &tx,
            idempotency_key,
            fingerprint,
            DnsOperation::Probe,
            zone_id,
            request_id,
            &snapshot,
        )?;
        tx.commit()?;

        Ok(DnsOperationReservation {
            zone,
            records,
            request_id,
            job_id: None,
            idempotent_replay: false,
        })
    }

    pub fn set_dns_operation_job_id(
        &mut self,
        idempotency_key: &str,
        job_id: Uuid,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE dns_operations SET job_id = ?1 WHERE idempotency_key = ?2",
            params![job_id.to_string(), idempotency_key],
        )?;
        Ok(())
    }

    pub fn complete_dns_operation(
        &mut self,
        request_id: Uuid,
        operation: DnsOperation,
        succeeded: bool,
        message: Option<&str>,
        now: &str,
    ) -> Result<bool, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        let zone_id_str: Option<String> = tx
            .query_row(
                "SELECT zone_id FROM dns_operations WHERE request_id = ?1 AND operation = ?2",
                params![request_id.to_string(), operation.as_str()],
                |row| row.get(0),
            )
            .optional()?;
        let Some(zone_id_str) = zone_id_str else {
            return Ok(false);
        };

        let active_request_id: Option<String> = tx
            .query_row(
                "SELECT operation_request_id FROM dns_zones WHERE zone_id = ?1",
                [&zone_id_str],
                |row| row.get(0),
            )
            .optional()?;
        if active_request_id.as_deref() != Some(&request_id.to_string()) {
            tx.commit()?;
            return Ok(true);
        }

        match operation {
            DnsOperation::ZoneApply => {
                if succeeded {
                    tx.execute(
                        "UPDATE dns_zones SET state = 'ready', last_error = NULL, updated_at = ?1 WHERE zone_id = ?2",
                        params![now, zone_id_str],
                    )?;
                } else {
                    tx.execute(
                        "UPDATE dns_zones SET state = 'failed', last_error = ?1, updated_at = ?2 WHERE zone_id = ?3",
                        params![message.unwrap_or("dns.zone.apply failed"), now, zone_id_str],
                    )?;
                }
            }
            DnsOperation::ZoneDelete => {
                if succeeded {
                    tx.execute("DELETE FROM dns_records WHERE zone_id = ?1", [&zone_id_str])?;
                    tx.execute("DELETE FROM dns_zones WHERE zone_id = ?1", [&zone_id_str])?;
                } else {
                    tx.execute(
                        "UPDATE dns_zones SET state = 'failed', last_error = ?1, updated_at = ?2 WHERE zone_id = ?3",
                        params![message.unwrap_or("dns.zone.delete failed"), now, zone_id_str],
                    )?;
                }
            }
            DnsOperation::Probe => {
                let probe_summary = if succeeded {
                    message.unwrap_or("propagated")
                } else {
                    message.unwrap_or("probe failed")
                };
                tx.execute(
                    "UPDATE dns_zones SET last_probe_result = ?1, updated_at = ?2 WHERE zone_id = ?3",
                    params![probe_summary, now, zone_id_str],
                )?;
            }
        }

        tx.commit()?;
        Ok(true)
    }
}

fn bump_zone_for_apply(
    tx: &Transaction<'_>,
    zone: &mut DnsZoneRecord,
    now: &str,
) -> Result<Uuid, StoreError> {
    let request_id = Uuid::new_v4();
    zone.serial = zone.serial.saturating_add(1);
    zone.state = DnsZoneState::Provisioning;
    zone.last_error = None;
    zone.updated_at = now.to_string();
    tx.execute(
        "UPDATE dns_zones SET serial = ?1, state = 'provisioning', last_error = NULL, updated_at = ?2, operation_request_id = ?3
         WHERE zone_id = ?4",
        params![
            zone.serial,
            zone.updated_at,
            request_id.to_string(),
            zone.id.to_string()
        ],
    )?;
    Ok(request_id)
}

fn load_zone_tx(tx: &Transaction<'_>, zone_id: Uuid) -> Result<DnsZoneRecord, StoreError> {
    tx.query_row(
        "SELECT zone_id, tenant_id, domain, serial, default_ttl, state, last_error, last_probe_result, created_at, updated_at
         FROM dns_zones WHERE zone_id = ?1",
        [zone_id.to_string()],
        row_to_zone,
    )
    .optional()?
    .ok_or(StoreError::NotFound(zone_id))
}

fn load_zone_records(
    conn: &rusqlite::Connection,
    zone_id: Uuid,
) -> Result<Vec<DnsRecordEntry>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT record_id, zone_id, tenant_id, name, record_type, value, ttl, priority, created_at, updated_at
         FROM dns_records WHERE zone_id = ?1 ORDER BY record_type, name, record_id",
    )?;
    let rows = stmt.query_map([zone_id.to_string()], row_to_record)?;
    let mut records = Vec::new();
    for row in rows {
        records.push(row?);
    }
    Ok(records)
}

fn find_existing_dns_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    expected_op: DnsOperation,
) -> Result<Option<DnsOperationReservation>, StoreError> {
    let row: Option<(String, String, String, String, Option<String>)> = tx
        .query_row(
            "SELECT fingerprint, operation, request_id, snapshot, job_id FROM dns_operations WHERE idempotency_key = ?1",
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
    let snapshot: DnsZoneWithRecords = serde_json::from_str(&snapshot_json)?;
    let request_id = Uuid::parse_str(&req_id_str)
        .map_err(|e| StoreError::Parse(format!("invalid dns request_id: {e}")))?;
    let job_id = job_id_str
        .map(|s| Uuid::parse_str(&s))
        .transpose()
        .map_err(|e| StoreError::Parse(format!("invalid dns job_id: {e}")))?;
    Ok(Some(DnsOperationReservation {
        zone: snapshot.zone,
        records: snapshot.records,
        request_id,
        job_id,
        idempotent_replay: true,
    }))
}

fn insert_dns_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
    operation: DnsOperation,
    zone_id: Uuid,
    request_id: Uuid,
    snapshot: &DnsZoneWithRecords,
) -> Result<(), StoreError> {
    let snapshot_json = serde_json::to_string(snapshot)?;
    tx.execute(
        "INSERT INTO dns_operations (idempotency_key, fingerprint, operation, zone_id, request_id, snapshot, job_id)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL)",
        params![
            idempotency_key,
            fingerprint,
            operation.as_str(),
            zone_id.to_string(),
            request_id.to_string(),
            snapshot_json,
        ],
    )?;
    Ok(())
}

fn row_to_zone(row: &Row<'_>) -> rusqlite::Result<DnsZoneRecord> {
    let zone_id_str: String = row.get(0)?;
    let tenant_id_str: String = row.get(1)?;
    let domain: String = row.get(2)?;
    let serial: u32 = row.get(3)?;
    let default_ttl: u32 = row.get(4)?;
    let state_str: String = row.get(5)?;
    let last_error: Option<String> = row.get(6)?;
    let last_probe_result: Option<String> = row.get(7)?;
    let created_at: String = row.get(8)?;
    let updated_at: String = row.get(9)?;

    let id = Uuid::parse_str(&zone_id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tenant_id = Uuid::parse_str(&tenant_id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let state = DnsZoneState::parse(&state_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(5, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(DnsZoneRecord {
        id,
        tenant_id,
        domain,
        serial,
        default_ttl,
        state,
        last_error,
        last_probe_result,
        created_at,
        updated_at,
    })
}

fn row_to_record(row: &Row<'_>) -> rusqlite::Result<DnsRecordEntry> {
    let record_id_str: String = row.get(0)?;
    let zone_id_str: String = row.get(1)?;
    let tenant_id_str: String = row.get(2)?;
    let name: String = row.get(3)?;
    let record_type: String = row.get(4)?;
    let value: String = row.get(5)?;
    let ttl: u32 = row.get(6)?;
    let priority: Option<i64> = row.get(7)?;
    let created_at: String = row.get(8)?;
    let updated_at: String = row.get(9)?;

    let id = Uuid::parse_str(&record_id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let zone_id = Uuid::parse_str(&zone_id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tenant_id = Uuid::parse_str(&tenant_id_str).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(DnsRecordEntry {
        id,
        zone_id,
        tenant_id,
        name,
        record_type,
        value,
        ttl,
        priority: priority.and_then(|p| u16::try_from(p).ok()),
        created_at,
        updated_at,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_zone_record_dkim_and_probe_lifecycle() {
        let db_path = std::env::temp_dir().join(format!("fp-dns-test-{}.db", Uuid::new_v4()));
        let mut store = ControlStore::new(&db_path).unwrap();
        let tenant = Uuid::new_v4();
        let now = "2026-11-17T14:00:00Z";

        let created = store
            .reserve_dns_zone_create("k1", "fp1", tenant, "example.com", 300, now)
            .unwrap();
        assert_eq!(created.zone.serial, 1);
        assert_eq!(created.zone.state, DnsZoneState::Provisioning);

        let replay = store
            .reserve_dns_zone_create("k1", "fp1", tenant, "example.com", 300, now)
            .unwrap();
        assert!(replay.idempotent_replay);
        assert_eq!(replay.zone.id, created.zone.id);

        assert!(store
            .complete_dns_operation(created.request_id, DnsOperation::ZoneApply, true, None, now)
            .unwrap());

        let rec = store
            .reserve_dns_record_create(
                "k2",
                "fp2",
                created.zone.id,
                DnsRecordMutation {
                    name: "www",
                    record_type: "A",
                    value: "203.0.113.10",
                    ttl: 300,
                    priority: None,
                },
                now,
            )
            .unwrap();
        assert_eq!(rec.zone.serial, 2);
        assert_eq!(rec.records.len(), 1);
        assert!(store
            .complete_dns_operation(rec.request_id, DnsOperation::ZoneApply, true, None, now)
            .unwrap());

        let dkim = store
            .reserve_dns_dkim_upsert(
                "k3",
                "fp3",
                created.zone.id,
                "default",
                "v=DKIM1; k=rsa; p=MIIBIjAN",
                300,
                now,
            )
            .unwrap();
        assert_eq!(dkim.zone.serial, 3);
        assert_eq!(dkim.records.len(), 2);
        assert!(store
            .complete_dns_operation(dkim.request_id, DnsOperation::ZoneApply, true, None, now)
            .unwrap());

        let probe = store
            .reserve_dns_probe("k4", "fp4", created.zone.id)
            .unwrap();
        assert!(store
            .complete_dns_operation(
                probe.request_id,
                DnsOperation::Probe,
                true,
                Some("DKIM TXT verified"),
                now
            )
            .unwrap());

        let loaded = store.get_dns_zone(created.zone.id).unwrap().unwrap();
        assert_eq!(loaded.zone.state, DnsZoneState::Ready);
        assert_eq!(
            loaded.zone.last_probe_result.as_deref(),
            Some("DKIM TXT verified")
        );
        let _ = std::fs::remove_file(db_path);
    }
}

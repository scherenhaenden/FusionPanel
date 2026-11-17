#![forbid(unsafe_code)]

use crate::{ControlStore, StoreError};
use rusqlite::{params, OptionalExtension, Row, Transaction};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseEngine {
    Postgresql,
    Mariadb,
}

impl DatabaseEngine {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Postgresql => "postgresql",
            Self::Mariadb => "mariadb",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "postgresql" => Ok(Self::Postgresql),
            "mariadb" => Ok(Self::Mariadb),
            other => Err(StoreError::Parse(format!(
                "unknown database engine '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseState {
    Creating,
    Ready,
    Failed,
    Deleting,
}

impl DatabaseState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Creating => "creating",
            Self::Ready => "ready",
            Self::Failed => "failed",
            Self::Deleting => "deleting",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "creating" => Ok(Self::Creating),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            "deleting" => Ok(Self::Deleting),
            other => Err(StoreError::Parse(format!(
                "unknown database state '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseUserState {
    Creating,
    Ready,
    Failed,
}

impl DatabaseUserState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Creating => "creating",
            Self::Ready => "ready",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Result<Self, StoreError> {
        match value {
            "creating" => Ok(Self::Creating),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            other => Err(StoreError::Parse(format!(
                "unknown database user state '{other}'"
            ))),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub engine: DatabaseEngine,
    pub state: DatabaseState,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DatabaseUserRecord {
    pub id: Uuid,
    pub database_id: Uuid,
    pub tenant_id: Uuid,
    pub username: String,
    pub state: DatabaseUserState,
    pub last_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DatabaseOperation {
    #[serde(rename = "database.apply")]
    Apply,
    #[serde(rename = "database.delete")]
    Delete,
    #[serde(rename = "database.user.create")]
    UserCreate,
}

impl DatabaseOperation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Apply => "database.apply",
            Self::Delete => "database.delete",
            Self::UserCreate => "database.user.create",
        }
    }
}

#[derive(Debug, Clone)]
pub struct DatabaseOperationReservation {
    pub database: DatabaseRecord,
    pub user: Option<DatabaseUserRecord>,
    pub request_id: Uuid,
    pub existing_job_id: Option<Uuid>,
    pub already_existed: bool,
}

impl ControlStore {
    pub fn list_databases_for_tenant(
        &self,
        tenant_id: Uuid,
    ) -> Result<Vec<DatabaseRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT database_id, tenant_id, name, engine, state, last_error, created_at, updated_at
             FROM databases
             WHERE tenant_id = ?1
             ORDER BY created_at DESC, name ASC",
        )?;
        let rows = stmt.query_map([tenant_id.to_string()], database_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn get_database_for_tenant(
        &self,
        database_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Option<DatabaseRecord>, StoreError> {
        self.conn
            .query_row(
                "SELECT database_id, tenant_id, name, engine, state, last_error, created_at, updated_at
                 FROM databases
                 WHERE database_id = ?1 AND tenant_id = ?2",
                params![database_id.to_string(), tenant_id.to_string()],
                database_from_row,
            )
            .optional()
            .map_err(StoreError::from)
    }

    pub fn list_database_users_for_tenant(
        &self,
        database_id: Uuid,
        tenant_id: Uuid,
    ) -> Result<Vec<DatabaseUserRecord>, StoreError> {
        let mut stmt = self.conn.prepare(
            "SELECT user_id, database_id, tenant_id, username, state, last_error, created_at, updated_at
             FROM database_users
             WHERE database_id = ?1 AND tenant_id = ?2
             ORDER BY created_at DESC, username ASC",
        )?;
        let rows = stmt.query_map(
            params![database_id.to_string(), tenant_id.to_string()],
            database_user_from_row,
        )?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    pub fn reserve_database_create(
        &mut self,
        tenant_id: Uuid,
        name: &str,
        engine: DatabaseEngine,
        idempotency_key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<DatabaseOperationReservation, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(existing) = load_database_operation(&tx, idempotency_key, fingerprint)? {
            tx.commit()?;
            return Ok(existing);
        }

        let name_exists = tx
            .query_row(
                "SELECT 1 FROM databases WHERE LOWER(name) = LOWER(?1) AND engine = ?2 LIMIT 1",
                params![name, engine.as_str()],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if name_exists {
            return Err(StoreError::DatabaseConflict(format!(
                "database '{name}' already exists for engine '{}'",
                engine.as_str()
            )));
        }

        let database_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let record = DatabaseRecord {
            id: database_id,
            tenant_id,
            name: name.to_string(),
            engine,
            state: DatabaseState::Creating,
            last_error: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        };

        tx.execute(
            "INSERT INTO databases (database_id, tenant_id, name, engine, state, last_error, created_at, updated_at, operation_request_id)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, ?6, ?7, ?8)",
            params![
                record.id.to_string(),
                record.tenant_id.to_string(),
                record.name,
                record.engine.as_str(),
                record.state.as_str(),
                record.created_at,
                record.updated_at,
                request_id.to_string(),
            ],
        )?;
        let snapshot = serde_json::to_string(&record)?;
        tx.execute(
            "INSERT INTO database_operations (idempotency_key, fingerprint, operation, database_id, request_id, snapshot, user_snapshot, job_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)",
            params![
                idempotency_key,
                fingerprint,
                DatabaseOperation::Apply.as_str(),
                record.id.to_string(),
                request_id.to_string(),
                snapshot,
            ],
        )?;
        tx.commit()?;

        Ok(DatabaseOperationReservation {
            database: record,
            user: None,
            request_id,
            existing_job_id: None,
            already_existed: false,
        })
    }

    pub fn reserve_database_delete(
        &mut self,
        database_id: Uuid,
        tenant_id: Uuid,
        idempotency_key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<Option<DatabaseOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(existing) = load_database_operation(&tx, idempotency_key, fingerprint)? {
            if existing.database.tenant_id != tenant_id {
                return Ok(None);
            }
            tx.commit()?;
            return Ok(Some(existing));
        }

        let current = tx
            .query_row(
                "SELECT database_id, tenant_id, name, engine, state, last_error, created_at, updated_at
                 FROM databases WHERE database_id = ?1 AND tenant_id = ?2",
                params![database_id.to_string(), tenant_id.to_string()],
                database_from_row,
            )
            .optional()?;
        let Some(mut record) = current else {
            return Ok(None);
        };

        if matches!(
            record.state,
            DatabaseState::Creating | DatabaseState::Deleting
        ) {
            return Err(StoreError::DatabaseOperationInProgress);
        }

        let request_id = Uuid::new_v4();
        record.state = DatabaseState::Deleting;
        record.last_error = None;
        record.updated_at = now.to_string();

        tx.execute(
            "UPDATE databases SET state = 'deleting', last_error = NULL, updated_at = ?1, operation_request_id = ?2
             WHERE database_id = ?3",
            params![now, request_id.to_string(), record.id.to_string()],
        )?;
        let snapshot = serde_json::to_string(&record)?;
        tx.execute(
            "INSERT INTO database_operations (idempotency_key, fingerprint, operation, database_id, request_id, snapshot, user_snapshot, job_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, NULL, NULL)",
            params![
                idempotency_key,
                fingerprint,
                DatabaseOperation::Delete.as_str(),
                record.id.to_string(),
                request_id.to_string(),
                snapshot,
            ],
        )?;
        tx.commit()?;

        Ok(Some(DatabaseOperationReservation {
            database: record,
            user: None,
            request_id,
            existing_job_id: None,
            already_existed: false,
        }))
    }

    pub fn reserve_database_user_create(
        &mut self,
        database_id: Uuid,
        tenant_id: Uuid,
        username: &str,
        idempotency_key: &str,
        fingerprint: &str,
        now: &str,
    ) -> Result<Option<DatabaseOperationReservation>, StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        if let Some(existing) = load_database_operation(&tx, idempotency_key, fingerprint)? {
            if existing.database.tenant_id != tenant_id {
                return Ok(None);
            }
            tx.commit()?;
            return Ok(Some(existing));
        }

        let current = tx
            .query_row(
                "SELECT database_id, tenant_id, name, engine, state, last_error, created_at, updated_at
                 FROM databases WHERE database_id = ?1 AND tenant_id = ?2",
                params![database_id.to_string(), tenant_id.to_string()],
                database_from_row,
            )
            .optional()?;
        let Some(db) = current else {
            return Ok(None);
        };

        if db.state != DatabaseState::Ready {
            return Err(StoreError::DatabaseOperationInProgress);
        }

        let username_exists = tx
            .query_row(
                "SELECT 1 FROM database_users WHERE database_id = ?1 AND LOWER(username) = LOWER(?2) LIMIT 1",
                params![db.id.to_string(), username],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .is_some();
        if username_exists {
            return Err(StoreError::DatabaseConflict(format!(
                "database user '{username}' already exists for '{}'",
                db.name
            )));
        }

        let user_id = Uuid::new_v4();
        let request_id = Uuid::new_v4();
        let user_record = DatabaseUserRecord {
            id: user_id,
            database_id: db.id,
            tenant_id,
            username: username.to_string(),
            state: DatabaseUserState::Creating,
            last_error: None,
            created_at: now.to_string(),
            updated_at: now.to_string(),
        };

        tx.execute(
            "INSERT INTO database_users (user_id, database_id, tenant_id, username, state, last_error, created_at, updated_at, operation_request_id)
             VALUES (?1, ?2, ?3, ?4, 'creating', NULL, ?5, ?6, ?7)",
            params![
                user_record.id.to_string(),
                user_record.database_id.to_string(),
                user_record.tenant_id.to_string(),
                user_record.username,
                user_record.created_at,
                user_record.updated_at,
                request_id.to_string(),
            ],
        )?;
        let snapshot = serde_json::to_string(&db)?;
        let user_snapshot = serde_json::to_string(&user_record)?;
        tx.execute(
            "INSERT INTO database_operations (idempotency_key, fingerprint, operation, database_id, request_id, snapshot, user_snapshot, job_id)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, NULL)",
            params![
                idempotency_key,
                fingerprint,
                DatabaseOperation::UserCreate.as_str(),
                db.id.to_string(),
                request_id.to_string(),
                snapshot,
                user_snapshot,
            ],
        )?;
        tx.commit()?;

        Ok(Some(DatabaseOperationReservation {
            database: db,
            user: Some(user_record),
            request_id,
            existing_job_id: None,
            already_existed: false,
        }))
    }

    pub fn attach_database_operation_job(
        &mut self,
        idempotency_key: &str,
        job_id: Uuid,
    ) -> Result<(), StoreError> {
        self.conn.execute(
            "UPDATE database_operations SET job_id = COALESCE(job_id, ?1) WHERE idempotency_key = ?2",
            params![job_id.to_string(), idempotency_key],
        )?;
        Ok(())
    }

    pub fn complete_database_operation(
        &mut self,
        operation: DatabaseOperation,
        database_id: Uuid,
        request_id: Uuid,
        succeeded: bool,
        error_text: Option<&str>,
        now: &str,
    ) -> Result<(), StoreError> {
        let tx = self
            .conn
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;
        match operation {
            DatabaseOperation::Apply => {
                let next_state = if succeeded { "ready" } else { "failed" };
                let next_error = if succeeded { None } else { error_text };
                tx.execute(
                    "UPDATE databases SET state = ?1, last_error = ?2, updated_at = ?3
                     WHERE database_id = ?4 AND operation_request_id = ?5",
                    params![
                        next_state,
                        next_error,
                        now,
                        database_id.to_string(),
                        request_id.to_string()
                    ],
                )?;
            }
            DatabaseOperation::Delete => {
                if succeeded {
                    let deleted = tx.execute(
                        "DELETE FROM databases WHERE database_id = ?1 AND operation_request_id = ?2",
                        params![database_id.to_string(), request_id.to_string()],
                    )?;
                    if deleted > 0 {
                        tx.execute(
                            "DELETE FROM database_users WHERE database_id = ?1",
                            [database_id.to_string()],
                        )?;
                    }
                } else {
                    tx.execute(
                        "UPDATE databases SET state = 'failed', last_error = ?1, updated_at = ?2
                         WHERE database_id = ?3 AND operation_request_id = ?4",
                        params![
                            error_text,
                            now,
                            database_id.to_string(),
                            request_id.to_string()
                        ],
                    )?;
                }
            }
            DatabaseOperation::UserCreate => {
                let next_state = if succeeded { "ready" } else { "failed" };
                let next_error = if succeeded { None } else { error_text };
                tx.execute(
                    "UPDATE database_users SET state = ?1, last_error = ?2, updated_at = ?3
                     WHERE database_id = ?4 AND operation_request_id = ?5",
                    params![
                        next_state,
                        next_error,
                        now,
                        database_id.to_string(),
                        request_id.to_string()
                    ],
                )?;
            }
        }
        tx.commit()?;
        Ok(())
    }
}

fn load_database_operation(
    tx: &Transaction<'_>,
    idempotency_key: &str,
    fingerprint: &str,
) -> Result<Option<DatabaseOperationReservation>, StoreError> {
    let existing = tx
        .query_row(
            "SELECT fingerprint, request_id, snapshot, user_snapshot, job_id
             FROM database_operations WHERE idempotency_key = ?1",
            [idempotency_key],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, Option<String>>(4)?,
                ))
            },
        )
        .optional()?;

    let Some((saved_fingerprint, request_id_raw, snapshot_raw, user_snapshot_raw, job_id_raw)) =
        existing
    else {
        return Ok(None);
    };

    if saved_fingerprint != fingerprint {
        return Err(StoreError::IdempotencyConflict);
    }

    let database: DatabaseRecord = serde_json::from_str(&snapshot_raw)?;
    let user = match user_snapshot_raw {
        Some(raw) => Some(serde_json::from_str(&raw)?),
        None => None,
    };
    let request_id = Uuid::parse_str(&request_id_raw)
        .map_err(|e| StoreError::Parse(format!("invalid request_id UUID: {e}")))?;
    let existing_job_id = job_id_raw
        .map(|raw| Uuid::parse_str(&raw))
        .transpose()
        .map_err(|e| StoreError::Parse(format!("invalid job_id UUID: {e}")))?;

    Ok(Some(DatabaseOperationReservation {
        database,
        user,
        request_id,
        existing_job_id,
        already_existed: true,
    }))
}

fn database_from_row(row: &Row<'_>) -> rusqlite::Result<DatabaseRecord> {
    let id_raw: String = row.get(0)?;
    let tenant_raw: String = row.get(1)?;
    let engine_raw: String = row.get(3)?;
    let state_raw: String = row.get(4)?;

    let id = Uuid::parse_str(&id_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tenant_id = Uuid::parse_str(&tenant_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let engine = DatabaseEngine::parse(&engine_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(3, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let state = DatabaseState::parse(&state_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(DatabaseRecord {
        id,
        tenant_id,
        name: row.get(2)?,
        engine,
        state,
        last_error: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

fn database_user_from_row(row: &Row<'_>) -> rusqlite::Result<DatabaseUserRecord> {
    let id_raw: String = row.get(0)?;
    let database_id_raw: String = row.get(1)?;
    let tenant_raw: String = row.get(2)?;
    let state_raw: String = row.get(4)?;

    let id = Uuid::parse_str(&id_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let database_id = Uuid::parse_str(&database_id_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(1, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let tenant_id = Uuid::parse_str(&tenant_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(2, rusqlite::types::Type::Text, Box::new(e))
    })?;
    let state = DatabaseUserState::parse(&state_raw).map_err(|e| {
        rusqlite::Error::FromSqlConversionFailure(4, rusqlite::types::Type::Text, Box::new(e))
    })?;

    Ok(DatabaseUserRecord {
        id,
        database_id,
        tenant_id,
        username: row.get(3)?,
        state,
        last_error: row.get(5)?,
        created_at: row.get(6)?,
        updated_at: row.get(7)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn database_and_user_lifecycle_is_idempotent_and_tenant_scoped() {
        let mut store = ControlStore::new_in_memory().unwrap();
        let tenant_a = Uuid::new_v4();
        let tenant_b = Uuid::new_v4();

        let created = store
            .reserve_database_create(
                tenant_a,
                "app_db",
                DatabaseEngine::Postgresql,
                "key-1",
                "fp-1",
                "2026-11-17T10:00:00Z",
            )
            .unwrap();
        assert!(!created.already_existed);
        assert_eq!(created.database.state, DatabaseState::Creating);

        store
            .complete_database_operation(
                DatabaseOperation::Apply,
                created.database.id,
                created.request_id,
                true,
                None,
                "2026-11-17T10:00:01Z",
            )
            .unwrap();

        let fetched = store
            .get_database_for_tenant(created.database.id, tenant_a)
            .unwrap()
            .unwrap();
        assert_eq!(fetched.state, DatabaseState::Ready);
        assert!(store
            .get_database_for_tenant(created.database.id, tenant_b)
            .unwrap()
            .is_none());

        let user_res = store
            .reserve_database_user_create(
                created.database.id,
                tenant_a,
                "app_user",
                "key-2",
                "fp-2",
                "2026-11-17T10:00:02Z",
            )
            .unwrap()
            .unwrap();
        store
            .complete_database_operation(
                DatabaseOperation::UserCreate,
                created.database.id,
                user_res.request_id,
                true,
                None,
                "2026-11-17T10:00:03Z",
            )
            .unwrap();

        let users = store
            .list_database_users_for_tenant(created.database.id, tenant_a)
            .unwrap();
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].state, DatabaseUserState::Ready);

        let delete_res = store
            .reserve_database_delete(
                created.database.id,
                tenant_a,
                "key-3",
                "fp-3",
                "2026-11-17T10:00:04Z",
            )
            .unwrap()
            .unwrap();
        store
            .complete_database_operation(
                DatabaseOperation::Delete,
                created.database.id,
                delete_res.request_id,
                true,
                None,
                "2026-11-17T10:00:05Z",
            )
            .unwrap();
        assert!(store
            .list_databases_for_tenant(tenant_a)
            .unwrap()
            .is_empty());
    }
}

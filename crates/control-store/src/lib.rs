#![forbid(unsafe_code)]

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use uuid::Uuid;

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

        tx.commit()?;
        Ok(())
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

    pub fn list_tenant_resources(&self, tenant_id: Uuid) -> Result<Vec<ResourceRecord>, StoreError> {
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
            let resource_id = Uuid::parse_str(&row.0).map_err(|e| StoreError::Parse(e.to_string()))?;
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
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

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
        let tx = self.conn.transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)?;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::os::unix::fs::PermissionsExt;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::SystemTime;
    use std::thread;

    static TEMP_COUNTER: AtomicUsize = AtomicUsize::new(0);

    struct TempDir {
        path: PathBuf,
    }

    impl TempDir {
        fn new() -> Self {
            let timestamp = SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).unwrap().as_nanos();
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
            store.create_resource(tenant_id, resource_id, OwnershipMode::TenantOwned, desired.clone()).unwrap();
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

        store.create_resource(tenant_id, resource_id, OwnershipMode::PlatformManaged, json!({})).unwrap();

        // Update with correct revision
        let new_rev = store.update_desired_state(resource_id, 1, json!({"status": "pending"})).unwrap();
        assert_eq!(new_rev, 2);

        // Update with wrong revision
        let err = store.update_desired_state(resource_id, 1, json!({"status": "active"})).unwrap_err();
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

        store.create_resource(tenant1, r1, OwnershipMode::TenantOwned, json!({})).unwrap();
        store.create_resource(tenant1, r2, OwnershipMode::PlatformManaged, json!({})).unwrap();
        store.create_resource(tenant2, r3, OwnershipMode::ExternallyManaged, json!({})).unwrap();

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
            store.create_resource(tenant_id, resource_id, OwnershipMode::TenantOwned, json!({})).unwrap();
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
                    },
                    Err(StoreError::Database(e)) if e.to_string().contains("database is locked") => {
                        thread::sleep(std::time::Duration::from_millis(50));
                        retries += 1;
                    },
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
                    },
                    Err(StoreError::Database(e)) if e.to_string().contains("database is locked") => {
                        thread::sleep(std::time::Duration::from_millis(50));
                        retries += 1;
                    },
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
            let user_version: u32 = conn.query_row("PRAGMA user_version", [], |row| row.get(0)).unwrap();
            assert_eq!(user_version, 1);
        }
    }
}

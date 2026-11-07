#![forbid(unsafe_code)]

#[path = "../src/backup_scheduler.rs"]
mod backup_scheduler;

use async_trait::async_trait;
use backup_scheduler::{
    BackupReceipt, Compression, DumpScheduler, EncryptionConfig, StorageBackend,
};
use chrono::Utc;
use databases::adapter::{
    ConnectionStrategy, CreateDbPlan, CreateUserPlan, DatabaseAdapter, DatabaseConfig,
    DatabaseEngine, DatabaseError, DbName, DbUser, DumpPlan, GrantPlan, RestorePlan,
    Result as DbResult, SecretString,
};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct MockAdapter {
    config: DatabaseConfig,
    fail_apply: bool,
}

#[async_trait]
impl DatabaseAdapter for MockAdapter {
async fn database_exists(&self, _name: &DbName) -> databases::adapter::Result<bool> { Ok(true) }
    async fn list_databases(&self) -> databases::adapter::Result<Vec<databases::adapter::ObservedDatabase>> { Ok(vec![]) }
    async fn user_exists(&self, _username: &DbUser) -> databases::adapter::Result<bool> { Ok(true) }
    fn engine(&self) -> DatabaseEngine {
        self.config.engine.clone()
    }
    fn config(&self) -> &DatabaseConfig {
        &self.config
    }
    async fn check_connection(&self) -> DbResult<()> {
        Ok(())
    }
    async fn database_exists(&self, _name: &DbName) -> DbResult<bool> {
        Ok(true)
    }
    async fn list_databases(&self) -> DbResult<Vec<databases::adapter::ObservedDatabase>> {
        Ok(vec![])
    }
    async fn user_exists(&self, _username: &DbUser) -> DbResult<bool> {
        Ok(true)
    }
    async fn plan_create_database(&self, name: DbName, owner: DbUser) -> DbResult<CreateDbPlan> {
        Ok(CreateDbPlan {
            db_name: name,
            owner,
        })
    }
    async fn apply_create_database(&self, _plan: &CreateDbPlan) -> DbResult<()> {
        Ok(())
    }
    async fn plan_create_user(
        &self,
        username: DbUser,
        password_ref: SecretString,
    ) -> DbResult<CreateUserPlan> {
        Ok(CreateUserPlan {
            username,
            password_ref,
        })
    }
    async fn apply_create_user(&self, _plan: &CreateUserPlan) -> DbResult<()> {
        Ok(())
    }
    async fn plan_grant(
        &self,
        db_name: DbName,
        username: DbUser,
        privileges: Vec<String>,
    ) -> DbResult<GrantPlan> {
        Ok(GrantPlan {
            db_name,
            username,
            privileges,
        })
    }
    async fn apply_grant(&self, _plan: &GrantPlan) -> DbResult<()> {
        Ok(())
    }
    async fn plan_dump(&self, db_name: DbName, output_path: String) -> DbResult<DumpPlan> {
        Ok(DumpPlan {
            db_name,
            output_path,
        })
    }

    async fn apply_dump(&self, plan: &DumpPlan) -> DbResult<()> {
        if self.fail_apply {
            return Err(DatabaseError::OperationFailed(
                "Simulated dump failure".to_string(),
            ));
        }
        // Mock writing a dummy uncompressed dump file
        std::fs::write(&plan.output_path, b"mock dump data").unwrap();
        Ok(())
    }

    async fn plan_restore(&self, db_name: DbName, input_path: String) -> DbResult<RestorePlan> {
        Ok(RestorePlan {
            db_name,
            input_path,
        })
    }
    async fn apply_restore(&self, _plan: &RestorePlan) -> DbResult<()> {
        Ok(())
    }
}

struct MockStorageBackend;

#[async_trait]
impl StorageBackend for MockStorageBackend {
    async fn upload(
        &self,
        local_path: &Path,
        remote_filename: &str,
    ) -> Result<BackupReceipt, String> {
        let bytes = std::fs::read(local_path).unwrap();
        let size = bytes.len() as u64;
        Ok(BackupReceipt {
            path_or_key: format!("s3://mock-bucket/{}", remote_filename),
            hash_sha256: format!("{:x}", Sha256::digest(&bytes)),
            size_bytes: size,
        })
    }
    async fn delete(&self, _remote_filename: &str) -> Result<(), String> {
        Ok(())
    }
}

fn create_mock_adapter(fail_apply: bool) -> Arc<dyn DatabaseAdapter> {
    Arc::new(MockAdapter {
        config: DatabaseConfig {
            engine: DatabaseEngine::PostgreSQL,
            strategy: ConnectionStrategy::Host { socket_path: None },
            host: "localhost".into(),
            port: 5432,
            admin_user: "admin".into(),
            admin_password_ref: SecretString("dummy".into()),
        },
        fail_apply,
    })
}

#[test]
fn test_cron_parsing() {
    let cron_expr = "0 0 0 * * * *";
    let adapter = create_mock_adapter(false);
    let scheduler = DumpScheduler::new(
        cron_expr,
        adapter,
        PathBuf::from("/tmp"),
        Compression::Gzip,
        None,
        Arc::new(MockStorageBackend),
        5,
    )
    .unwrap();

    let next_run = scheduler.next_run().unwrap();
    assert!(next_run > Utc::now());
}

#[test]
fn test_invalid_cron_parsing() {
    let cron_expr = "invalid cron";
    let adapter = create_mock_adapter(false);
    let scheduler = DumpScheduler::new(
        cron_expr,
        adapter,
        PathBuf::from("/tmp"),
        Compression::Gzip,
        None,
        Arc::new(MockStorageBackend),
        5,
    );
    assert!(scheduler.is_err());
}

#[tokio::test]
async fn test_execute_dump_mock_success() {
    let adapter = create_mock_adapter(false);

    let temp_dir = tempfile::tempdir().unwrap();
    let scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        adapter,
        temp_dir.path().to_path_buf(),
        Compression::Zstd,
        None,
        Arc::new(MockStorageBackend),
        5,
    )
    .unwrap();

    let res = scheduler.execute_dump("mydb", "test_db_dump.sql.zst").await;
    assert!(res.is_ok());
    let receipt = res.unwrap();
    let remote_name = receipt
        .path_or_key
        .strip_prefix("s3://mock-bucket/")
        .unwrap();
    assert!(remote_name.ends_with("-test_db_dump.sql.zst"));
    assert_eq!(
        receipt.size_bytes,
        std::fs::metadata(
            temp_dir
                .path()
                .join(".fusionpanel-backups")
                .join(remote_name)
        )
        .unwrap()
        .len()
    );
    assert_eq!(
        receipt.hash_sha256,
        format!(
            "{:x}",
            Sha256::digest(
                std::fs::read(
                    temp_dir
                        .path()
                        .join(".fusionpanel-backups")
                        .join(remote_name)
                )
                .unwrap()
            )
        )
    );
}

#[tokio::test]
async fn encrypted_dump_fails_closed_without_key_resolution_and_restore_support() {
    let adapter = create_mock_adapter(false);
    let temp_dir = tempfile::tempdir().unwrap();
    let scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        adapter,
        temp_dir.path().to_path_buf(),
        Compression::Zstd,
        Some(EncryptionConfig {
            key_ref: "key://test".into(),
        }),
        Arc::new(MockStorageBackend),
        5,
    )
    .unwrap();

    let error = scheduler
        .execute_dump("mydb", "test.sql.zst")
        .await
        .unwrap_err();
    assert!(error.to_string().contains("BLOCKED"));
    assert!(!temp_dir.path().join(".fusionpanel-backups").exists());
}

#[tokio::test]
async fn rejects_path_components_in_backup_filename() {
    let adapter = create_mock_adapter(false);
    let temp_dir = tempfile::tempdir().unwrap();
    let scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        adapter,
        temp_dir.path().to_path_buf(),
        Compression::Gzip,
        None,
        Arc::new(MockStorageBackend),
        5,
    )
    .unwrap();

    assert!(scheduler
        .execute_dump("mydb", "../escape.sql")
        .await
        .is_err());
    assert!(!temp_dir.path().join("escape.sql").exists());
}

#[tokio::test]
async fn test_execute_dump_mock_failure_cleans_up() {
    let adapter = create_mock_adapter(true);

    let temp_dir = tempfile::tempdir().unwrap();
    let scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        adapter,
        temp_dir.path().to_path_buf(),
        Compression::Zstd,
        None,
        Arc::new(MockStorageBackend),
        5,
    )
    .unwrap();

    let res = scheduler.execute_dump("mydb", "test_db_dump.sql.zst").await;
    assert!(res.is_err());

    // Ensure uncompressed file is not left behind on failure
    assert!(
        std::fs::read_dir(temp_dir.path().join(".fusionpanel-backups"))
            .unwrap()
            .next()
            .is_none()
    );
}

#[tokio::test]
async fn test_local_retention() {
    let adapter = create_mock_adapter(false);

    let temp_dir = tempfile::tempdir().unwrap();

    // Files in the scheduler-owned directory are eligible for its retention policy.
    let backup_dir = temp_dir.path().join(".fusionpanel-backups");
    std::fs::create_dir(&backup_dir).unwrap();
    std::fs::write(backup_dir.join("old1.sql.zst"), b"1").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    std::fs::write(backup_dir.join("old2.sql.zst"), b"2").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    std::fs::write(backup_dir.join("old3.sql.zst"), b"3").unwrap();

    let scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        adapter,
        temp_dir.path().to_path_buf(),
        Compression::Zstd,
        None,
        Arc::new(MockStorageBackend),
        2, // retain only 2 files
    )
    .unwrap();

    let res = scheduler.execute_dump("mydb", "test_db_dump.sql.zst").await;
    assert!(res.is_ok());

    let entries: Vec<_> = std::fs::read_dir(backup_dir).unwrap().collect();
    assert_eq!(entries.len(), 2); // The new dump + the newest old dump
}

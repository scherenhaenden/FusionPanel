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
use std::path::{Path, PathBuf};
use std::sync::Arc;

struct MockAdapter {
    config: DatabaseConfig,
    fail_apply: bool,
}

#[async_trait]
impl DatabaseAdapter for MockAdapter {
    fn engine(&self) -> DatabaseEngine {
        self.config.engine.clone()
    }
    fn config(&self) -> &DatabaseConfig {
        &self.config
    }
    async fn check_connection(&self) -> DbResult<()> {
        Ok(())
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
        let size = local_path.metadata().unwrap().len();
        Ok(BackupReceipt {
            path_or_key: format!("s3://mock-bucket/{}", remote_filename),
            hash_sha256: "mock-hash-from-s3".to_string(), // In real app, S3 upload might compute ETag, we compute sha256 locally
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
        Some(EncryptionConfig {
            key_ref: "secret-key".to_string(),
        }),
        Arc::new(MockStorageBackend),
        5,
    )
    .unwrap();

    let res = scheduler.execute_dump("mydb", "test_db_dump.sql.zst").await;
    assert!(res.is_ok());
    let receipt = res.unwrap();
    assert_eq!(receipt.path_or_key, "s3://mock-bucket/test_db_dump.sql.zst");
    assert!(temp_dir
        .path()
        .join("test_db_dump.sql.zst.encrypted")
        .exists());
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
    assert!(!temp_dir
        .path()
        .join("test_db_dump.sql.zst.uncompressed")
        .exists());
    assert!(!temp_dir.path().join("test_db_dump.sql.zst").exists());
}

#[tokio::test]
async fn test_local_retention() {
    let adapter = create_mock_adapter(false);

    let temp_dir = tempfile::tempdir().unwrap();

    // Create 3 old files
    std::fs::write(temp_dir.path().join("old1.sql.zst"), b"1").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    std::fs::write(temp_dir.path().join("old2.sql.zst"), b"2").unwrap();
    std::thread::sleep(std::time::Duration::from_millis(10));
    std::fs::write(temp_dir.path().join("old3.sql.zst"), b"3").unwrap();

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

    let entries: Vec<_> = std::fs::read_dir(temp_dir.path()).unwrap().collect();
    assert_eq!(entries.len(), 2); // The new dump + the newest old dump
}

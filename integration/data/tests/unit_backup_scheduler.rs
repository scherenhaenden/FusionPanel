#![forbid(unsafe_code)]

#[path = "../src/backup_scheduler.rs"]
mod backup_scheduler;

use async_trait::async_trait;
use backup_scheduler::{Compression, DumpScheduler};
use chrono::Utc;
use databases::adapter::{
    ConnectionStrategy, CreateDbPlan, CreateUserPlan, DatabaseAdapter, DatabaseConfig,
    DatabaseEngine, DbName, DbUser, DumpPlan, GrantPlan, RestorePlan, Result as DbResult,
    SecretString,
};
use std::path::PathBuf;
use std::sync::Arc;

struct MockAdapter {
    config: DatabaseConfig,
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

fn create_mock_adapter() -> Arc<dyn DatabaseAdapter> {
    Arc::new(MockAdapter {
        config: DatabaseConfig {
            engine: DatabaseEngine::PostgreSQL,
            strategy: ConnectionStrategy::Host { socket_path: None },
            host: "localhost".into(),
            port: 5432,
            admin_user: "admin".into(),
            admin_password_ref: SecretString("dummy".into()),
        },
    })
}

#[test]
fn test_cron_parsing() {
    let cron_expr = "0 0 0 * * * *";
    let adapter = create_mock_adapter();
    let scheduler =
        DumpScheduler::new(cron_expr, adapter, PathBuf::from("/tmp"), Compression::Gzip).unwrap();

    let next_run = scheduler.next_run().unwrap();
    assert!(next_run > Utc::now());
}

#[test]
fn test_invalid_cron_parsing() {
    let cron_expr = "invalid cron";
    let adapter = create_mock_adapter();
    let scheduler =
        DumpScheduler::new(cron_expr, adapter, PathBuf::from("/tmp"), Compression::Gzip);
    assert!(scheduler.is_err());
}

#[tokio::test]
async fn test_execute_dump_mock() {
    let adapter = create_mock_adapter();

    let temp_dir = tempfile::tempdir().unwrap();
    let scheduler = DumpScheduler::new(
        "0 0 0 * * * *",
        adapter,
        temp_dir.path().to_path_buf(),
        Compression::Zstd,
    )
    .unwrap();

    let res = scheduler.execute_dump("mydb", "test_db_dump.sql.zst").await;
    assert!(res.is_ok());
    let (path, _hash) = res.unwrap();
    assert!(path.exists());
}

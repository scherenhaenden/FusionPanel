#![forbid(unsafe_code)]

use migration::executor::{ExecutionStatus, MigrationAdapter, MigrationExecutor};
use migration::inventory::InventoryItemType;
use migration::planner::{MigrationPlan, PlanAction, PlanActionType};
use tempfile::TempDir;

struct IntegrationAdapter {
    root_dir: std::path::PathBuf,
    fail_on_database: Option<String>,
}

impl IntegrationAdapter {
    fn new(root_dir: std::path::PathBuf) -> Self {
        Self {
            root_dir,
            fail_on_database: None,
        }
    }

    fn user_path(&self, name: &str) -> std::path::PathBuf {
        self.root_dir.join(format!("user_{}", name))
    }

    fn db_path(&self, name: &str) -> std::path::PathBuf {
        self.root_dir.join(format!("db_{}", name))
    }

    fn site_path(&self, name: &str) -> std::path::PathBuf {
        self.root_dir.join(format!("site_{}", name))
    }
}

impl MigrationAdapter for IntegrationAdapter {
    fn create_user(&mut self, name: &str) -> Result<(), String> {
        let path = self.user_path(name);
        std::fs::write(&path, "user data").map_err(|e| e.to_string())
    }

    fn create_database(&mut self, name: &str) -> Result<(), String> {
        if let Some(ref fail_name) = self.fail_on_database {
            if fail_name == name {
                return Err("Simulated database failure".to_string());
            }
        }
        let path = self.db_path(name);
        std::fs::write(&path, "db data").map_err(|e| e.to_string())
    }

    fn create_site(&mut self, name: &str) -> Result<(), String> {
        let path = self.site_path(name);
        std::fs::write(&path, "site data").map_err(|e| e.to_string())
    }

    fn delete_user(&mut self, name: &str) -> Result<(), String> {
        let path = self.user_path(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn delete_database(&mut self, name: &str) -> Result<(), String> {
        let path = self.db_path(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn delete_site(&mut self, name: &str) -> Result<(), String> {
        let path = self.site_path(name);
        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

fn generate_plan() -> MigrationPlan {
    MigrationPlan {
        actions: vec![
            PlanAction {
                action: PlanActionType::Create,
                item_id: "u1".to_string(),
                item_name: "admin".to_string(),
                item_type: InventoryItemType::User,
                reason: None,
            },
            PlanAction {
                action: PlanActionType::Create,
                item_id: "d1".to_string(),
                item_name: "test_db".to_string(),
                item_type: InventoryItemType::Database,
                reason: None,
            },
            PlanAction {
                action: PlanActionType::Create,
                item_id: "s1".to_string(),
                item_name: "example.com".to_string(),
                item_type: InventoryItemType::Site,
                reason: None,
            },
        ],
        is_dry_run: false,
        decommission_allowed: false,
    }
}

#[test]
fn integration_successful_execution() {
    let temp_dir = TempDir::new().unwrap();
    let mut adapter = IntegrationAdapter::new(temp_dir.path().to_path_buf());
    let mut executor = MigrationExecutor::new(&mut adapter);
    
    let plan = generate_plan();
    let result = executor.execute(&plan);
    
    assert!(result.success);
    assert_eq!(result.records.len(), 3);
    assert!(result.records.iter().all(|r| r.status == ExecutionStatus::Success));
    
    // Verify files created on disk
    assert!(adapter.user_path("admin").exists());
    assert!(adapter.db_path("test_db").exists());
    assert!(adapter.site_path("example.com").exists());
}

#[test]
fn integration_failed_execution_with_rollback() {
    let temp_dir = TempDir::new().unwrap();
    let mut adapter = IntegrationAdapter::new(temp_dir.path().to_path_buf());
    
    // Cause failure on the database creation step
    adapter.fail_on_database = Some("test_db".to_string());
    
    let mut executor = MigrationExecutor::new(&mut adapter);
    
    let plan = generate_plan();
    let result = executor.execute(&plan);
    
    assert!(!result.success);
    assert_eq!(result.records.len(), 2); // user and db attempted
    
    // The user should have been successfully created initially, then rolled back
    assert_eq!(result.records[0].item_name, "admin");
    assert_eq!(result.records[0].status, ExecutionStatus::RolledBack);
    
    // The database creation failed
    assert_eq!(result.records[1].item_name, "test_db");
    assert_eq!(result.records[1].status, ExecutionStatus::Failed);
    
    // Verify files removed from disk via rollback
    assert!(!adapter.user_path("admin").exists());
    assert!(!adapter.db_path("test_db").exists());
    assert!(!adapter.site_path("example.com").exists());
}

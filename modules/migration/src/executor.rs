use crate::inventory::InventoryItemType;
use crate::planner::{MigrationPlan, PlanActionType};
use serde::{Deserialize, Serialize};

pub trait MigrationAdapter {
    fn create_user(&mut self, name: &str) -> Result<(), String>;
    fn create_database(&mut self, name: &str) -> Result<(), String>;
    fn create_site(&mut self, name: &str) -> Result<(), String>;

    fn delete_user(&mut self, name: &str) -> Result<(), String>;
    fn delete_database(&mut self, name: &str) -> Result<(), String>;
    fn delete_site(&mut self, name: &str) -> Result<(), String>;
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ExecutionStatus {
    Success,
    Failed,
    RolledBack,
    RollbackFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ExecutionRecord {
    pub item_id: String,
    pub item_name: String,
    pub item_type: InventoryItemType,
    pub status: ExecutionStatus,
    pub error_message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationResult {
    pub success: bool,
    pub records: Vec<ExecutionRecord>,
    pub decommission_attested: bool,
}

pub struct MigrationExecutor<'a, A: MigrationAdapter> {
    adapter: &'a mut A,
}

impl<'a, A: MigrationAdapter> MigrationExecutor<'a, A> {
    pub fn new(adapter: &'a mut A) -> Self {
        Self { adapter }
    }

    pub fn execute(&mut self, plan: &MigrationPlan) -> MigrationResult {
        if plan.is_dry_run {
            return MigrationResult {
                success: true,
                records: Vec::new(),
                decommission_attested: plan.decommission_allowed,
            };
        }

        let mut records = Vec::new();
        let mut executed_steps = Vec::new();
        let mut execution_failed = false;

        for action in &plan.actions {
            if action.action != PlanActionType::Create {
                continue;
            }

            let result = match action.item_type {
                InventoryItemType::User => self.adapter.create_user(&action.item_name),
                InventoryItemType::Database => self.adapter.create_database(&action.item_name),
                InventoryItemType::Site => self.adapter.create_site(&action.item_name),
                InventoryItemType::Unknown(_) => Ok(()),
            };

            match result {
                Ok(_) => {
                    executed_steps.push(action.clone());
                    records.push(ExecutionRecord {
                        item_id: action.item_id.clone(),
                        item_name: action.item_name.clone(),
                        item_type: action.item_type.clone(),
                        status: ExecutionStatus::Success,
                        error_message: None,
                    });
                }
                Err(err) => {
                    execution_failed = true;
                    records.push(ExecutionRecord {
                        item_id: action.item_id.clone(),
                        item_name: action.item_name.clone(),
                        item_type: action.item_type.clone(),
                        status: ExecutionStatus::Failed,
                        error_message: Some(err),
                    });
                    break;
                }
            }
        }

        if execution_failed {
            for action in executed_steps.iter().rev() {
                let rollback_result = match action.item_type {
                    InventoryItemType::User => self.adapter.delete_user(&action.item_name),
                    InventoryItemType::Database => self.adapter.delete_database(&action.item_name),
                    InventoryItemType::Site => self.adapter.delete_site(&action.item_name),
                    InventoryItemType::Unknown(_) => Ok(()),
                };

                if let Some(record) = records.iter_mut().find(|r| r.item_id == action.item_id) {
                    if rollback_result.is_ok() {
                        record.status = ExecutionStatus::RolledBack;
                    } else {
                        record.status = ExecutionStatus::RollbackFailed;
                        if let Err(e) = rollback_result {
                            record.error_message = Some(format!("Rollback failed: {}", e));
                        }
                    }
                }
            }

            return MigrationResult {
                success: false,
                records,
                decommission_attested: false,
            };
        }

        MigrationResult {
            success: true,
            records,
            decommission_attested: plan.decommission_allowed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::planner::{PlanAction, PlanActionType};

    struct MockAdapter {
        user_fail: bool,
        db_fail: bool,
        site_fail: bool,
        rollback_user_fail: bool,
        calls: Vec<String>,
    }

    impl MockAdapter {
        fn new() -> Self {
            Self {
                user_fail: false,
                db_fail: false,
                site_fail: false,
                rollback_user_fail: false,
                calls: Vec::new(),
            }
        }
    }

    impl MigrationAdapter for MockAdapter {
        fn create_user(&mut self, name: &str) -> Result<(), String> {
            self.calls.push(format!("create_user:{}", name));
            if self.user_fail { Err("user fail".to_string()) } else { Ok(()) }
        }
        fn create_database(&mut self, name: &str) -> Result<(), String> {
            self.calls.push(format!("create_database:{}", name));
            if self.db_fail { Err("db fail".to_string()) } else { Ok(()) }
        }
        fn create_site(&mut self, name: &str) -> Result<(), String> {
            self.calls.push(format!("create_site:{}", name));
            if self.site_fail { Err("site fail".to_string()) } else { Ok(()) }
        }
        fn delete_user(&mut self, name: &str) -> Result<(), String> {
            self.calls.push(format!("delete_user:{}", name));
            if self.rollback_user_fail { Err("rollback fail".to_string()) } else { Ok(()) }
        }
        fn delete_database(&mut self, name: &str) -> Result<(), String> {
            self.calls.push(format!("delete_database:{}", name));
            Ok(())
        }
        fn delete_site(&mut self, name: &str) -> Result<(), String> {
            self.calls.push(format!("delete_site:{}", name));
            Ok(())
        }
    }

    fn create_plan() -> MigrationPlan {
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
                    item_name: "db1".to_string(),
                    item_type: InventoryItemType::Database,
                    reason: None,
                },
                PlanAction {
                    action: PlanActionType::Create,
                    item_id: "s1".to_string(),
                    item_name: "site1".to_string(),
                    item_type: InventoryItemType::Site,
                    reason: None,
                },
            ],
            is_dry_run: false,
            decommission_allowed: true,
        }
    }

    #[test]
    fn test_successful_execution() {
        let mut adapter = MockAdapter::new();
        let mut executor = MigrationExecutor::new(&mut adapter);
        let plan = create_plan();
        
        let result = executor.execute(&plan);
        assert!(result.success);
        assert!(result.decommission_attested);
        assert_eq!(result.records.len(), 3);
        assert!(result.records.iter().all(|r| r.status == ExecutionStatus::Success));
        
        assert_eq!(adapter.calls, vec![
            "create_user:admin",
            "create_database:db1",
            "create_site:site1",
        ]);
    }

    #[test]
    fn test_execution_failure_and_rollback() {
        let mut adapter = MockAdapter::new();
        adapter.site_fail = true;
        let mut executor = MigrationExecutor::new(&mut adapter);
        let plan = create_plan();
        
        let result = executor.execute(&plan);
        assert!(!result.success);
        assert!(!result.decommission_attested);
        
        assert_eq!(result.records.len(), 3);
        assert_eq!(result.records[0].status, ExecutionStatus::RolledBack);
        assert_eq!(result.records[1].status, ExecutionStatus::RolledBack);
        assert_eq!(result.records[2].status, ExecutionStatus::Failed);
        
        assert_eq!(adapter.calls, vec![
            "create_user:admin",
            "create_database:db1",
            "create_site:site1",
            "delete_database:db1",
            "delete_user:admin",
        ]);
    }

    #[test]
    fn test_rollback_failure() {
        let mut adapter = MockAdapter::new();
        adapter.db_fail = true;
        adapter.rollback_user_fail = true;
        
        let mut executor = MigrationExecutor::new(&mut adapter);
        let plan = create_plan();
        
        let result = executor.execute(&plan);
        assert!(!result.success);
        
        assert_eq!(result.records.len(), 2);
        assert_eq!(result.records[0].status, ExecutionStatus::RollbackFailed); 
        assert_eq!(result.records[1].status, ExecutionStatus::Failed);
        
        assert_eq!(adapter.calls, vec![
            "create_user:admin",
            "create_database:db1",
            "delete_user:admin",
        ]);
    }

    #[test]
    fn test_dry_run() {
        let mut adapter = MockAdapter::new();
        let mut executor = MigrationExecutor::new(&mut adapter);
        let mut plan = create_plan();
        plan.is_dry_run = true;
        
        let result = executor.execute(&plan);
        assert!(result.success);
        assert!(result.records.is_empty());
        assert!(adapter.calls.is_empty());
    }
}

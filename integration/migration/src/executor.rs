use migration::inventory::MigrationInventory;
use migration::planner::{PlanActionType, Planner};
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    Init,
    Exported,
    Transferred,
    Imported,
    Cutover,
    RolledBack,
    Failed,
    Blocked,
}

pub struct MigrationExecutor {
    work_dir: PathBuf,
    state: MigrationState,
}

impl MigrationExecutor {
    pub fn new(work_dir: PathBuf) -> Self {
        Self {
            work_dir,
            state: MigrationState::Init,
        }
    }

    pub fn execute(&mut self, inventory: &MigrationInventory) -> Result<(), String> {
        let plan = Planner::create_plan(inventory, false);

        // Check for unsupported or conflicting actions
        for action in &plan.actions {
            if action.action == PlanActionType::Unsupported
                || action.action == PlanActionType::Conflict
            {
                self.state = MigrationState::Failed;
                return Err(format!(
                    "Cannot execute plan: item {} is {:?}",
                    action.item_id, action.action
                ));
            }
        }

        let recovery_marker = self.work_dir.join(".recovery-marker");

        if recovery_marker.exists() {
            self.state = MigrationState::Blocked;
            return Err(format!(
                "BLOCKED: recovery marker at {} requires a qualified source/target rollback adapter; marker retained",
                recovery_marker.display()
            ));
        }

        self.state = MigrationState::Blocked;
        Err("BLOCKED: no qualified source export, target import, or rollback adapter is configured; migration was not started".to_string())
    }

    pub fn rollback(&mut self) -> Result<(), String> {
        self.state = MigrationState::Blocked;
        Err("BLOCKED: no qualified source/target rollback adapter is configured; existing recovery state was not changed".to_string())
    }

    pub fn get_state(&self) -> &MigrationState {
        &self.state
    }
}

#[cfg(test)]
mod tests {
    use super::{MigrationExecutor, MigrationState};
    use migration::inventory::MigrationInventory;
    use std::fs;

    #[test]
    fn execute_fails_closed_when_runtime_adapters_are_missing() {
        let work_dir = tempfile::tempdir().unwrap();
        let mut executor = MigrationExecutor::new(work_dir.path().to_path_buf());
        let inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());

        let error = executor.execute(&inventory).unwrap_err();
        assert!(error.starts_with("BLOCKED:"));
        assert_eq!(*executor.get_state(), MigrationState::Blocked);
        assert!(!work_dir.path().join(".recovery-marker").exists());
    }

    #[test]
    fn execute_preserves_recovery_marker_without_qualified_rollback() {
        let work_dir = tempfile::tempdir().unwrap();
        let marker = work_dir.path().join(".recovery-marker");
        fs::write(&marker, "checkpoint").unwrap();
        let mut executor = MigrationExecutor::new(work_dir.path().to_path_buf());
        let inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());

        let error = executor.execute(&inventory).unwrap_err();
        assert!(error.starts_with("BLOCKED:"));
        assert_eq!(*executor.get_state(), MigrationState::Blocked);
        assert!(marker.exists());
    }
}

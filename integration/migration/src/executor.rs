use migration::inventory::MigrationInventory;
use migration::planner::{Planner, PlanActionType};
use std::path::PathBuf;
use std::fs;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MigrationState {
    Init,
    Exported,
    Transferred,
    Imported,
    Cutover,
    RolledBack,
    Failed,
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
            if action.action == PlanActionType::Unsupported || action.action == PlanActionType::Conflict {
                self.state = MigrationState::Failed;
                return Err(format!("Cannot execute plan: item {} is {:?}", action.item_id, action.action));
            }
        }

        let recovery_marker = self.work_dir.join(".recovery-marker");

        if recovery_marker.exists() {
            // Simulate recovery by rolling back
            self.rollback()?;
            return Err("Recovered from previous crash".to_string());
        }

        // Write marker for crash resilience
        fs::write(&recovery_marker, "checkpoint").map_err(|e| e.to_string())?;

        self.state = MigrationState::Exported;
        // Simulate export

        self.state = MigrationState::Transferred;
        // Simulate transfer

        self.state = MigrationState::Imported;
        // Simulate import

        self.state = MigrationState::Cutover;

        // Remove marker upon success
        fs::remove_file(&recovery_marker).map_err(|e| e.to_string())?;

        Ok(())
    }

    pub fn rollback(&mut self) -> Result<(), String> {
        self.state = MigrationState::RolledBack;
        let recovery_marker = self.work_dir.join(".recovery-marker");
        if recovery_marker.exists() {
            fs::remove_file(&recovery_marker).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    pub fn get_state(&self) -> &MigrationState {
        &self.state
    }
}

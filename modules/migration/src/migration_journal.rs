#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum MigrationStepType {
    Discover,
    DumpDatabase,
    RsyncWebroot,
    ProvisionVhost,
    ImportDatabase,
    CutoverDns,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum StepState {
    Pending,
    InProgress,
    Completed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationStep {
    pub step_type: MigrationStepType,
    pub state: StepState,
    pub start_timestamp: Option<u64>,
    pub completion_timestamp: Option<u64>,
    pub error_description: Option<String>,
}

impl MigrationStep {
    pub fn new(step_type: MigrationStepType) -> Self {
        Self {
            step_type,
            state: StepState::Pending,
            start_timestamp: None,
            completion_timestamp: None,
            error_description: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RollbackAction {
    RevertDiscover,
    RevertDumpDatabase,
    RevertRsyncWebroot,
    RevertProvisionVhost,
    RevertImportDatabase,
    RevertCutoverDns,
}

impl MigrationStepType {
    pub fn rollback_action(&self) -> RollbackAction {
        match self {
            MigrationStepType::Discover => RollbackAction::RevertDiscover,
            MigrationStepType::DumpDatabase => RollbackAction::RevertDumpDatabase,
            MigrationStepType::RsyncWebroot => RollbackAction::RevertRsyncWebroot,
            MigrationStepType::ProvisionVhost => RollbackAction::RevertProvisionVhost,
            MigrationStepType::ImportDatabase => RollbackAction::RevertImportDatabase,
            MigrationStepType::CutoverDns => RollbackAction::RevertCutoverDns,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationJournal {
    pub steps: Vec<MigrationStep>,
}

impl MigrationJournal {
    pub fn new(step_types: Vec<MigrationStepType>) -> Self {
        let steps = step_types.into_iter().map(MigrationStep::new).collect();
        Self { steps }
    }

    pub fn start_step(&mut self, index: usize) {
        if let Some(step) = self.steps.get_mut(index) {
            step.state = StepState::InProgress;
            step.start_timestamp = Some(current_timestamp());
            step.completion_timestamp = None;
            step.error_description = None;
        }
    }

    pub fn complete_step(&mut self, index: usize) {
        if let Some(step) = self.steps.get_mut(index) {
            step.state = StepState::Completed;
            step.completion_timestamp = Some(current_timestamp());
            step.error_description = None;
        }
    }

    pub fn fail_step(&mut self, index: usize, error: String) {
        if let Some(step) = self.steps.get_mut(index) {
            step.state = StepState::Failed;
            step.completion_timestamp = Some(current_timestamp());
            step.error_description = Some(error);
        }
    }

    pub fn reset_step(&mut self, index: usize) {
        if let Some(step) = self.steps.get_mut(index) {
            step.state = StepState::Pending;
            step.start_timestamp = None;
            step.completion_timestamp = None;
            step.error_description = None;
        }
    }

    pub fn next_pending_or_failed_step(&self) -> Option<(usize, MigrationStepType)> {
        self.steps.iter().enumerate().find_map(|(idx, step)| {
            if step.state == StepState::Pending || step.state == StepState::Failed {
                Some((idx, step.step_type.clone()))
            } else {
                None
            }
        })
    }

    pub fn generate_rollback_plan(&self) -> Vec<RollbackAction> {
        let mut rollback_plan = Vec::new();
        // Iterate in reverse to ensure the rollback sequence is inverted as required.
        for step in self.steps.iter().rev() {
            if step.state == StepState::Completed
                || step.state == StepState::InProgress
                || step.state == StepState::Failed
            {
                rollback_plan.push(step.step_type.rollback_action());
            }
        }
        rollback_plan
    }
}

fn current_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

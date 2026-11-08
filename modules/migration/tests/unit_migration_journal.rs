#![forbid(unsafe_code)]

use migration::migration_journal::{
    MigrationJournal, MigrationStepType, RollbackAction, StepState,
};

#[test]
fn test_state_transitions() {
    let mut journal = MigrationJournal::new(vec![
        MigrationStepType::Discover,
        MigrationStepType::DumpDatabase,
    ]);

    // Initial state
    assert_eq!(journal.steps[0].state, StepState::Pending);
    assert_eq!(journal.steps[1].state, StepState::Pending);

    // Start step
    journal.start_step(0);
    assert_eq!(journal.steps[0].state, StepState::InProgress);
    assert!(journal.steps[0].start_timestamp.is_some());
    assert!(journal.steps[0].completion_timestamp.is_none());
    assert!(journal.steps[0].error_description.is_none());

    // Complete step
    journal.complete_step(0);
    assert_eq!(journal.steps[0].state, StepState::Completed);
    assert!(journal.steps[0].completion_timestamp.is_some());

    // Fail step
    journal.start_step(1);
    journal.fail_step(1, "Connection refused".to_string());
    assert_eq!(journal.steps[1].state, StepState::Failed);
    assert!(journal.steps[1].completion_timestamp.is_some());
    assert_eq!(
        journal.steps[1].error_description,
        Some("Connection refused".to_string())
    );

    // Reset step
    journal.reset_step(1);
    assert_eq!(journal.steps[1].state, StepState::Pending);
    assert!(journal.steps[1].start_timestamp.is_none());
    assert!(journal.steps[1].completion_timestamp.is_none());
    assert!(journal.steps[1].error_description.is_none());
}

#[test]
fn test_checkpoint_recovery() {
    let mut journal = MigrationJournal::new(vec![
        MigrationStepType::Discover,
        MigrationStepType::DumpDatabase,
        MigrationStepType::RsyncWebroot,
    ]);

    // All pending, next is 0
    assert_eq!(
        journal.next_pending_or_failed_step(),
        Some((0, MigrationStepType::Discover))
    );

    // Complete 0, next is 1
    journal.start_step(0);
    journal.complete_step(0);
    assert_eq!(
        journal.next_pending_or_failed_step(),
        Some((1, MigrationStepType::DumpDatabase))
    );

    // Fail 1, next is 1
    journal.start_step(1);
    journal.fail_step(1, "Error".to_string());
    assert_eq!(
        journal.next_pending_or_failed_step(),
        Some((1, MigrationStepType::DumpDatabase))
    );

    // Reset 1, complete 1, next is 2
    journal.reset_step(1);
    journal.start_step(1);
    journal.complete_step(1);
    assert_eq!(
        journal.next_pending_or_failed_step(),
        Some((2, MigrationStepType::RsyncWebroot))
    );

    // Complete 2, none remaining
    journal.start_step(2);
    journal.complete_step(2);
    assert_eq!(journal.next_pending_or_failed_step(), None);
}

#[test]
fn test_rollback_plan_generation() {
    let mut journal = MigrationJournal::new(vec![
        MigrationStepType::Discover,
        MigrationStepType::DumpDatabase,
        MigrationStepType::RsyncWebroot,
        MigrationStepType::ProvisionVhost,
        MigrationStepType::ImportDatabase,
        MigrationStepType::CutoverDns,
    ]);

    journal.start_step(0);
    journal.complete_step(0);
    journal.start_step(1);
    journal.complete_step(1);
    journal.start_step(2);
    // 2 is InProgress
    journal.start_step(3);
    journal.fail_step(3, "Failed to provision vhost".to_string());

    // Only steps 0, 1, 2, 3 have been started/completed/failed
    // Thus the rollback should be reverting 3, 2, 1, 0 (in reverse order)
    let plan = journal.generate_rollback_plan();

    assert_eq!(plan.len(), 4);
    assert_eq!(plan[0], RollbackAction::RevertProvisionVhost);
    assert_eq!(plan[1], RollbackAction::RevertRsyncWebroot);
    assert_eq!(plan[2], RollbackAction::RevertDumpDatabase);
    assert_eq!(plan[3], RollbackAction::RevertDiscover);
}

#[test]
fn test_rollback_plan_inverted_sequence() {
    let mut journal = MigrationJournal::new(vec![
        MigrationStepType::Discover,
        MigrationStepType::DumpDatabase,
        MigrationStepType::RsyncWebroot,
    ]);

    journal.start_step(0);
    journal.complete_step(0);

    journal.start_step(1);
    journal.complete_step(1);

    journal.start_step(2);
    journal.fail_step(2, "Sync failed".to_string());

    let plan = journal.generate_rollback_plan();

    assert_eq!(plan, vec![
        RollbackAction::RevertRsyncWebroot,
        RollbackAction::RevertDumpDatabase,
        RollbackAction::RevertDiscover,
    ]);
}

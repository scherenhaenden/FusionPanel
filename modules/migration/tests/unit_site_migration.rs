#![forbid(unsafe_code)]

use migration::site_migration::{MigrationError, MigrationPhase, SiteMigrationPlanner};

#[test]
fn test_successful_planning_and_ordering() {
    let planner = SiteMigrationPlanner::new(
        "example.com".to_string(),
        1000,
        1500, // 1000 * 1.2 = 1200 < 1500
        true,
    );

    let plan = planner.plan().expect("Plan should succeed");

    assert_eq!(plan.len(), 8);
    assert_eq!(plan[0], MigrationPhase::PreCopyFiles);
    assert_eq!(plan[1], MigrationPhase::SetupTargetVhost);
    assert_eq!(plan[2], MigrationPhase::ExportDatabase);
    assert_eq!(plan[3], MigrationPhase::SyncDatabase);
    assert_eq!(plan[4], MigrationPhase::DeltaSyncFiles);
    assert_eq!(plan[5], MigrationPhase::DnsCutover);
    assert_eq!(plan[6], MigrationPhase::Verification);
    assert_eq!(plan[7], MigrationPhase::FinalizeCleanup);
}

#[test]
fn test_insufficient_space_rejection() {
    let planner = SiteMigrationPlanner::new(
        "example.com".to_string(),
        1000,
        1100, // 1000 * 1.2 = 1200 > 1100
        true,
    );

    let result = planner.plan();
    assert!(matches!(result, Err(MigrationError::InsufficientSpace)));
}

#[test]
fn test_invalid_domain_rejection() {
    // Empty domain
    let planner = SiteMigrationPlanner::new("".to_string(), 1000, 1500, true);
    assert!(matches!(planner.plan(), Err(MigrationError::InvalidDomain)));

    // No dot
    let planner2 = SiteMigrationPlanner::new("examplecom".to_string(), 1000, 1500, true);
    assert!(matches!(planner2.plan(), Err(MigrationError::InvalidDomain)));

    // Starts with hyphen
    let planner3 = SiteMigrationPlanner::new("-example.com".to_string(), 1000, 1500, true);
    assert!(matches!(planner3.plan(), Err(MigrationError::InvalidDomain)));

    // Invalid chars
    let planner4 = SiteMigrationPlanner::new("ex@mple.com".to_string(), 1000, 1500, true);
    assert!(matches!(planner4.plan(), Err(MigrationError::InvalidDomain)));
}

#[test]
fn test_invalid_user_mapping_rejection() {
    let planner = SiteMigrationPlanner::new(
        "example.com".to_string(),
        1000,
        1500,
        false, // Invalid user mapping
    );

    let result = planner.plan();
    assert!(matches!(result, Err(MigrationError::InvalidUserMapping)));
}

#[test]
fn test_partial_rollback_sequence_generation() {
    let completed_phases = vec![
        MigrationPhase::PreCopyFiles,
        MigrationPhase::SetupTargetVhost,
        MigrationPhase::ExportDatabase,
    ];

    let rollback_sequence = SiteMigrationPlanner::generate_rollback_sequence(&completed_phases);

    assert_eq!(rollback_sequence.len(), 3);
    assert_eq!(rollback_sequence[0], MigrationPhase::ExportDatabase);
    assert_eq!(rollback_sequence[1], MigrationPhase::SetupTargetVhost);
    assert_eq!(rollback_sequence[2], MigrationPhase::PreCopyFiles);
}

#![forbid(unsafe_code)]

use migration::discovery::CloudPanelDiscovery;
use migration::inventory::MigrationInventory;
use migration::planner::{Planner, ReviewEvidence};
use std::path::Path;

#[test]
fn test_migration_discovery_and_planning_e2e() {
    // According to the STRICT ZERO-MOCK RULE, we must not generate synthetic databases.
    // We expect a real `cloudpanel.db` fixture to be provided via environment or existing in a fixtures dir.
    // For this E2E test, we check if the fixture exists. If not, we fail-closed with BLOCKED.

    let fixture_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/cloudpanel.db");

    if !fixture_path.exists() {
        // Output format required by python test runner to detect blocked tests
        println!("BLOCKED: Missing required real SQLite database fixture for migration test at {}", fixture_path.display());
        panic!("BLOCKED: Missing required real SQLite database fixture for migration test at {}", fixture_path.display());
    }

    // 1. MIG-01 & MIG-02: CloudPanel import and SQLite discovery
    let inventory = CloudPanelDiscovery::discover(&fixture_path).expect("Failed to discover inventory from real fixture");

    // We can't make strong assertions on the *content* because it's a real user fixture,
    // but we can assert that it parsed successfully and has the right type.
    assert_eq!(inventory.source_panel, "CloudPanel");
    assert!(!inventory.source_version.is_empty());

    // 2. MIG-08: Migration dry-run planning and pre-flight validation
    let plan = Planner::create_plan(&inventory, true);
    assert_eq!(plan.is_dry_run, true);
    assert!(!plan.actions.is_empty());

    // 3. MIG-10: Whole-server rollback drill (Simulate failure during handoff)
    let review_evidence = ReviewEvidence {
        execution_completed: true,
        target_verified: true,
        restore_verified: true,
        handoff_completed: false, // Simulating failure
    };

    let review_plan = Planner::create_review_plan(&inventory, false, Some(&review_evidence));
    assert_eq!(review_plan.decommission_allowed, false);

    // 4. MIG-12: Downtime control (Ensure decommission fails for empty inventory)
    let empty_inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
    let empty_review_plan = Planner::create_review_plan(&empty_inventory, false, Some(&ReviewEvidence {
        execution_completed: true,
        target_verified: true,
        restore_verified: true,
        handoff_completed: true,
    }));
    assert_eq!(empty_review_plan.decommission_allowed, false); // Fails due to empty inventory
}

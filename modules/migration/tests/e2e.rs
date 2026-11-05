use migration::discovery::CloudPanelDiscovery;
use std::path::Path;

/// End-to-end verification cases
/// E2E requires a full sanitized server copy and execution of plan import without changing target.
/// Currently BLOCKED because runtime prerequisites (e.g. system access for actual migration)
/// are not available yet.

#[test]
#[ignore = "BLOCKED: Missing prerequisite sanitized_server.sqlite. Cannot run full E2E discovery against real server dump."]
fn e2e_migration_plan_generation_blocked() {
    // We would use an actual sanitized server database here
    let server_db = Path::new("tests/fixtures/sanitized_server.sqlite");

    if !server_db.exists() {
        // A blocked unrun case is not a pass in acceptance, but we mark it as ignored/blocked.
        // The test runner shouldn't execute this because of #[ignore] if the fixture is missing.
        panic!("BLOCKED: Missing prerequisite sanitized_server.sqlite");
    }

    let inventory = CloudPanelDiscovery::discover(server_db).unwrap();
    let plan = migration::planner::Planner::create_plan(&inventory, true);

    // E2E asserts on plan length or specific properties of the real database
    assert!(plan.actions.len() > 0);
}

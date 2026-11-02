#[path = "common/blocked_driver.rs"]
mod blocked_driver;

#[test]
fn deploy_known_commit_serve_rollback_verify() {
    blocked_driver::run(
        "FUSIONPANEL_DEPLOYMENTS_E2E_DRIVER",
        "deploy-known-commit-serve-rollback-verify",
        "live production HTTP server, real network cutover, and an executable fixture driver",
    );
}

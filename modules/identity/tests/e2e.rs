#[path = "common/blocked_driver.rs"]
mod blocked_driver;

/// E2E verification test case:
/// Denied cross-tenant API requests after S25. If runtime prerequisites are not available yet,
/// provide executable case definitions and record BLOCKED with the exact dependency.
/// A blocked/unrun case is not a pass. Final acceptance requires actual observed user outcome.
#[test]
fn denied_cross_tenant_api_requests() {
    blocked_driver::run(
        "FUSIONPANEL_IDENTITY_E2E_DRIVER",
        "denied-cross-tenant-api-requests",
        "the integrated S25 HTTP API server, real multi-tenant sessions, and an executable E2E fixture driver",
    );
}

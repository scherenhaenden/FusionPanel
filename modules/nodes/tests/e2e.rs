#![forbid(unsafe_code)]

#[path = "common/blocked_driver.rs"]
mod blocked_driver;

#[test]
fn enroll_two_nodes_revoke_one_and_reject_its_requests() {
    blocked_driver::run(
        "FUSIONPANEL_NODES_E2E_DRIVER",
        "enroll-two-revoke-one-reject-requests",
        "the integrated API and node agent, two disposable nodes, signed requests, and an executable E2E fixture driver",
    );
}

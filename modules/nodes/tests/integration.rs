#![forbid(unsafe_code)]

#[path = "common/blocked_driver.rs"]
mod blocked_driver;

#[test]
fn two_local_node_identities_use_signed_transport() {
    blocked_driver::run(
        "FUSIONPANEL_NODES_INTEGRATION_DRIVER",
        "two-identities-signed-transport",
        "two disposable local service identities, the signed node transport, and an executable fixture driver",
    );
}

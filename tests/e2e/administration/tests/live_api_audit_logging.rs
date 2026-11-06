#![forbid(unsafe_code)]

#[path = "live_api/common.rs"]
mod common;

#[tokio::test]
async fn test_audit_logging_immutability() {
    let (client, api_url) = common::client_and_target().unwrap_or_else(|error| panic!("{error}"));
    common::require_health(&client, &api_url)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    common::require_route(&client, &api_url, "/api/v1/audit")
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    // This package has no isolated controller for restarting the target and
    // proving its audit store survived. Do not write an audit event and then
    // report a permanent BLOCKED result.
    panic!("BLOCKED: an owned restart-capable audit fixture is required before creating an event");
}

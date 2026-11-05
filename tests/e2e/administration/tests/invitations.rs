#![forbid(unsafe_code)]

#[path = "common/mod.rs"]
mod common;

#[tokio::test]
async fn test_team_member_invitation_flow() {
    let (client, api_url) = common::client_and_target().unwrap_or_else(|error| panic!("{error}"));
    common::require_health(&client, &api_url)
        .await
        .unwrap_or_else(|error| panic!("{error}"));
    for path in [
        "/api/v1/invitations",
        "/api/v1/invitations/test-probe",
        "/api/v1/invitations/test-probe/accept",
    ] {
        common::require_route(&client, &api_url, path)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
    }

    // The current fixture cannot provision an authenticated invitee identity
    // or guarantee cleanup of a pending invitation. Avoid creating a resource
    // whose revocation and acceptance cannot be verified safely.
    panic!("BLOCKED: a disposable invitation fixture with an authenticated invitee and cleanup path is not configured");
}

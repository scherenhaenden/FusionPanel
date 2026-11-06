#![forbid(unsafe_code)]

#[path = "live_api/common.rs"]
mod common;

use reqwest::StatusCode;
use serde_json::json;

#[tokio::test]
async fn test_organization_tenant_quotas() {
    let (client, api_url) = common::client_and_target().unwrap_or_else(|error| panic!("{error}"));
    common::require_health(&client, &api_url)
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    let auth_token = std::env::var("FUSIONPANEL_TOKEN").unwrap_or_else(|_| {
        panic!("BLOCKED: FUSIONPANEL_TOKEN is required for the disposable target")
    });
    for path in [
        "/api/v1/tenants",
        "/api/v1/tenants/test-probe/allocate",
        "/api/v1/tenants/test-probe",
    ] {
        common::require_route(&client, &api_url, path)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
    }

    let tenant_id = uuid::Uuid::new_v4().to_string();
    let create_res = client
        .post(format!("{api_url}/api/v1/tenants"))
        .bearer_auth(&auth_token)
        .json(&json!({"id": tenant_id, "quota_limit": 5}))
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: tenant fixture creation failed"));
    if !create_res.status().is_success() {
        panic!(
            "BLOCKED: tenant fixture creation returned {}",
            create_res.status()
        );
    }

    let mut tasks = Vec::new();
    for _ in 0..10 {
        let client = client.clone();
        let api_url = api_url.clone();
        let tenant_id = tenant_id.clone();
        let auth_token = auth_token.clone();
        tasks.push(tokio::spawn(async move {
            client
                .post(format!("{api_url}/api/v1/tenants/{tenant_id}/allocate"))
                .bearer_auth(auth_token)
                .json(&json!({"amount": 1}))
                .send()
                .await
                .map(|response| response.status())
                .map_err(|_| "allocation request failed".to_string())
        }));
    }

    let mut statuses = Vec::with_capacity(tasks.len());
    let mut task_errors = Vec::new();
    for task in tasks {
        match task.await {
            Ok(Ok(status)) => statuses.push(status),
            Ok(Err(error)) => task_errors.push(error),
            Err(error) => task_errors.push(format!("allocation task failed: {error}")),
        }
    }

    // Delete the synthetic tenant before asserting the measured race result so
    // a failed assertion does not leave the fixture behind.
    let cleanup = client
        .delete(format!("{api_url}/api/v1/tenants/{tenant_id}"))
        .bearer_auth(&auth_token)
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: could not clean up the synthetic tenant"));
    if !cleanup.status().is_success() {
        panic!(
            "BLOCKED: synthetic tenant cleanup returned {}",
            cleanup.status()
        );
    }

    assert!(
        task_errors.is_empty(),
        "quota requests failed: {task_errors:?}"
    );
    let admitted = statuses.iter().filter(|status| status.is_success()).count();
    let rejected = statuses
        .iter()
        .filter(|status| {
            matches!(
                **status,
                StatusCode::CONFLICT | StatusCode::TOO_MANY_REQUESTS
            )
        })
        .count();
    assert_eq!(
        admitted, 5,
        "quota race admitted an unexpected number of allocations"
    );
    assert_eq!(
        rejected, 5,
        "quota race did not reject all excess allocations"
    );
    assert_eq!(
        statuses.len(),
        10,
        "quota race did not produce ten HTTP responses"
    );
}

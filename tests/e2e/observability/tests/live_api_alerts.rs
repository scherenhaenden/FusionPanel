#![forbid(unsafe_code)]
#![forbid(unsafe_code)]

#[path = "live_api/common.rs"]
mod common;

use serde_json::{json, Value};

#[tokio::test]
async fn test_monitoring_alerts() {
    let (client, api_url) = common::client_and_target().unwrap_or_else(|error| panic!("{error}"));
    common::require_health(&client, &api_url)
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    let auth_token = std::env::var("FUSIONPANEL_TOKEN").unwrap_or_else(|_| {
        panic!("BLOCKED: FUSIONPANEL_TOKEN is required for the disposable target")
    });
    for path in [
        "/api/v1/debug/trigger-alert",
        "/api/v1/alerts/history",
        "/api/v1/debug/repair-alert",
    ] {
        common::require_route(&client, &api_url, path)
            .await
            .unwrap_or_else(|error| panic!("{error}"));
    }

    let fixture_id = uuid::Uuid::new_v4().to_string();
    let trigger = client
        .post(format!("{api_url}/api/v1/debug/trigger-alert"))
        .bearer_auth(&auth_token)
        .json(&json!({"type": "outage", "fixture_id": fixture_id}))
        .send()
        .await;
    let trigger_result = match trigger {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Ok((status, body))
        }
        Err(error) => Err(error.to_string()),
    };

    // Always attempt the fixture's explicit repair after triggering, before
    // making assertions that could otherwise leave an outage active.
    let history = client
        .get(format!("{api_url}/api/v1/alerts/history"))
        .bearer_auth(&auth_token)
        .send()
        .await;
    let history_result = match history {
        Ok(response) => {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            Ok((status, body))
        }
        Err(error) => Err(error.to_string()),
    };
    let repair = client
        .post(format!("{api_url}/api/v1/debug/repair-alert"))
        .bearer_auth(&auth_token)
        .json(&json!({"type": "outage", "fixture_id": fixture_id}))
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: disposable outage repair request failed"));
    if !repair.status().is_success() {
        panic!(
            "BLOCKED: disposable outage repair returned {}",
            repair.status()
        );
    }
    common::require_health(&client, &api_url)
        .await
        .unwrap_or_else(|error| panic!("{error}"));

    let (trigger_status, trigger_body) = trigger_result.unwrap_or_else(|error| {
        panic!("BLOCKED: could not verify outage trigger response: {error}")
    });
    assert!(
        trigger_status.is_success(),
        "failed to create outage: {trigger_status}"
    );
    let (history_status, history_body) = history_result
        .unwrap_or_else(|error| panic!("BLOCKED: could not read alert history: {error}"));
    assert!(
        history_status.is_success(),
        "failed to read alert history: {history_status}"
    );

    let trigger_json: Value = serde_json::from_str(&trigger_body)
        .unwrap_or_else(|_| panic!("BLOCKED: outage response did not identify a durable alert"));
    let alert_id = trigger_json
        .get("alert_id")
        .or_else(|| trigger_json.get("id"))
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("BLOCKED: outage response did not include an alert identifier"));
    assert!(
        history_body.contains(alert_id),
        "alert history did not contain the created alert ID"
    );
    assert!(
        history_body.contains("outage"),
        "alert history did not include the outage type"
    );
}

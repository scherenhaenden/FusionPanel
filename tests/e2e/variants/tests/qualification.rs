#![forbid(unsafe_code)]

use reqwest::{Client, StatusCode, Url};
use serde_json::json;
use std::{env, time::Duration};

fn live_target() -> (Client, Url, String) {
    if env::var("FUSIONPANEL_E2E_DISPOSABLE").as_deref() != Ok("1") {
        panic!("BLOCKED: set FUSIONPANEL_E2E_DISPOSABLE=1 only for an owned disposable target");
    }

    let raw_url = env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| {
        panic!("BLOCKED: FUSIONPANEL_API_URL must name the disposable loopback API")
    });
    let base = Url::parse(&raw_url)
        .unwrap_or_else(|_| panic!("BLOCKED: FUSIONPANEL_API_URL is not a valid URL"));
    if base.scheme() != "http"
        || !matches!(base.host_str(), Some("127.0.0.1" | "localhost"))
        || base.port().is_none()
        || !base.username().is_empty()
        || base.password().is_some()
        || base.query().is_some()
        || base.fragment().is_some()
        || !matches!(base.path(), "" | "/")
    {
        panic!("BLOCKED: variant tests require an explicit HTTP loopback API URL with a port");
    }

    let token = env::var("FUSIONPANEL_TOKEN").unwrap_or_else(|_| {
        panic!("BLOCKED: FUSIONPANEL_TOKEN is required for the disposable API")
    });
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap_or_else(|_| panic!("BLOCKED: could not create a bounded API client"));
    (client, base, token)
}

async fn require_health(client: &Client, base: &Url, token: &str) {
    let url = base.join("/v1/health").unwrap();
    let response = client
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: disposable FusionPanel API health check failed"));
    if !response.status().is_success() {
        panic!(
            "BLOCKED: disposable FusionPanel API health returned {}",
            response.status()
        );
    }
}

fn variant_site_id() -> u64 {
    env::var("FUSIONPANEL_E2E_VARIANT_SITE_ID")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|id| *id > 0)
        .unwrap_or_else(|| {
            panic!("BLOCKED: FUSIONPANEL_E2E_VARIANT_SITE_ID must name a disposable test site")
        })
}

#[tokio::test]
async fn test_variant_capabilities_api_query() {
    let (client, base, token) = live_target();
    require_health(&client, &base, &token).await;

    let response = client
        .get(base.join("/api/v1/variants").unwrap())
        .bearer_auth(&token)
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: variant registry endpoint is unreachable"));
    if response.status() == StatusCode::NOT_FOUND
        || response.status() == StatusCode::NOT_IMPLEMENTED
    {
        panic!("BLOCKED: variant registry endpoint is not implemented");
    }
    assert!(
        response.status().is_success(),
        "variant registry returned {}",
        response.status()
    );

    let variants: serde_json::Value = response
        .json()
        .await
        .expect("variant registry must return JSON");
    let variants = variants
        .as_array()
        .unwrap_or_else(|| panic!("variant registry response must be an array"));
    for capability in ["ADM-02", "ADM-03", "HOST-01", "CTR-01", "BAK-04", "MIG-09"] {
        assert!(
            variants
                .iter()
                .any(|variant| variant["capability_id"] == capability),
            "variant registry omitted {capability}"
        );
    }
}

#[tokio::test]
async fn test_variant_mode_transitions() {
    let (client, base, token) = live_target();
    require_health(&client, &base, &token).await;
    let site_id = variant_site_id();

    let response = client
        .post(
            base.join(&format!("/api/v1/sites/{site_id}/variants/transition"))
                .unwrap(),
        )
        .bearer_auth(&token)
        .json(&json!({
            "capability_id": "HOST-01",
            "mode_id": "upstream_reverse_proxy"
        }))
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: variant transition endpoint is unreachable"));
    if response.status() == StatusCode::NOT_FOUND
        || response.status() == StatusCode::NOT_IMPLEMENTED
    {
        panic!("BLOCKED: variant transition endpoint is not implemented");
    }
    assert!(
        response.status().is_success(),
        "variant transition returned {}",
        response.status()
    );

    let result: serde_json::Value = response
        .json()
        .await
        .expect("variant transition must return JSON");
    assert!(
        result["job_id"].is_string(),
        "transition must return its durable job ID"
    );
}

#[tokio::test]
async fn test_invalid_variant_transitions_rejected() {
    let (client, base, token) = live_target();
    require_health(&client, &base, &token).await;
    let site_id = variant_site_id();

    let response = client
        .post(
            base.join(&format!("/api/v1/sites/{site_id}/variants/transition"))
                .unwrap(),
        )
        .bearer_auth(&token)
        .json(&json!({
            "capability_id": "CTR-01",
            "mode_id": "unsupported_legacy_mode"
        }))
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: variant transition endpoint is unreachable"));
    if response.status() == StatusCode::NOT_FOUND
        || response.status() == StatusCode::NOT_IMPLEMENTED
    {
        panic!("BLOCKED: variant transition endpoint is not implemented");
    }
    assert!(
        matches!(
            response.status(),
            StatusCode::BAD_REQUEST | StatusCode::UNPROCESSABLE_ENTITY
        ),
        "invalid variant transition was not rejected: {}",
        response.status()
    );
}

#[tokio::test]
async fn test_variant_data_persistence() {
    let (client, base, token) = live_target();
    require_health(&client, &base, &token).await;

    let response = client
        .get(base.join("/api/v1/backups/latest/manifest").unwrap())
        .bearer_auth(&token)
        .send()
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: backup manifest endpoint is unreachable"));
    if response.status() == StatusCode::NOT_FOUND
        || response.status() == StatusCode::NOT_IMPLEMENTED
    {
        panic!("BLOCKED: backup manifest endpoint is not implemented");
    }
    assert!(
        response.status().is_success(),
        "backup manifest returned {}",
        response.status()
    );

    let manifest: serde_json::Value = response
        .json()
        .await
        .expect("backup manifest must return JSON");
    assert!(
        manifest["variants"].is_object(),
        "backup manifest must include persisted variants"
    );
}

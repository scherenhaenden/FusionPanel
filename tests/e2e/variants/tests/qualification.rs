#![forbid(unsafe_code)]

use std::process::Command;

fn ensure_runtime() {
    let output = Command::new("docker").arg("--version").output();
    if output.is_err() || !output.unwrap().status.success() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: docker not available.");
    }
}

#[tokio::test]
async fn test_variant_capabilities_api_query() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let res = reqwest::get(&format!("{}/api/v1/variants", api_url)).await;
    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    let res = res.unwrap();
    if !res.status().is_success() {
        panic!("BLOCKED: incomplete variant registry endpoints.");
    }

    let variants: serde_json::Value = res.json().await.expect("Valid JSON");

    // Validate supported management/storage/deployment/TLS choices
    let required_capabilities = ["ADM-02", "ADM-03", "HOST-01", "CTR-01", "BAK-04", "MIG-09"];

    let variants_array = variants.as_array().expect("Variants should be an array");
    for cap in required_capabilities {
        assert!(
            variants_array.iter().any(|v| v["capability_id"] == cap),
            "Variant registry must preserve supported choice for capability {}", cap
        );
    }
}

#[tokio::test]
async fn test_variant_mode_transitions() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let client = reqwest::Client::new();

    // Assume a site resource exists to test transition
    let res = client.post(&format!("{}/api/v1/sites/1/variants/transition", api_url))
        .json(&serde_json::json!({
            "capability_id": "HOST-01",
            "mode_id": "upstream_reverse_proxy"
        }))
        .send().await;

    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    let res = res.unwrap();
    if !res.status().is_success() {
        panic!("BLOCKED: incomplete variant transition behavior.");
    }

    // Verify transition success and job returned
    let body: serde_json::Value = res.json().await.expect("Valid JSON");
    assert!(body["job_id"].is_string(), "Transition must execute as a durable audited job");
}

#[tokio::test]
async fn test_invalid_variant_transitions_rejected() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let client = reqwest::Client::new();

    // Attempt incompatible/unsafe transition (e.g., container release to release-directory switch without proper setup)
    let res = client.post(&format!("{}/api/v1/sites/1/variants/transition", api_url))
        .json(&serde_json::json!({
            "capability_id": "CTR-01",
            "mode_id": "unsupported_legacy_mode"
        }))
        .send().await;

    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    let res = res.unwrap();
    if res.status().is_success() {
        panic!("BLOCKED: missing negative transition cases.");
    }

    assert_eq!(res.status(), reqwest::StatusCode::BAD_REQUEST, "Invalid transition must be denied");
}

#[tokio::test]
async fn test_variant_data_persistence() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    // Verify backup export contains the variant
    let res = reqwest::get(&format!("{}/api/v1/backups/latest/manifest", api_url)).await;

    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    let res = res.unwrap();
    if !res.status().is_success() {
        panic!("BLOCKED: missing variant data persistence cases.");
    }

    let manifest: serde_json::Value = res.json().await.expect("Valid JSON");
    assert!(manifest["variants"].is_object(), "Backup boundary must span transition artifacts and persist modes");
}

#![forbid(unsafe_code)]

use std::process::Command;

fn ensure_runtime() {
    let output = Command::new("docker").arg("--version").output();
    if output.is_err() || !output.unwrap().status.success() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: docker not available.");
    }
}

#[tokio::test]
#[ignore = "BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at http://127.0.0.1:8080"]
async fn test_variant_capabilities_api_query() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let res = reqwest::get(&api_url).await;
    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    panic!("BLOCKED: incomplete variant registry endpoints.");
}

#[tokio::test]
#[ignore = "BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at http://127.0.0.1:8080"]
async fn test_variant_mode_transitions() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let res = reqwest::get(&api_url).await;
    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    panic!("BLOCKED: incomplete variant transition behavior.");
}

#[tokio::test]
#[ignore = "BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at http://127.0.0.1:8080"]
async fn test_invalid_variant_transitions_rejected() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let res = reqwest::get(&api_url).await;
    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    panic!("BLOCKED: missing negative transition cases.");
}

#[tokio::test]
#[ignore = "BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at http://127.0.0.1:8080"]
async fn test_variant_data_persistence() {
    ensure_runtime();
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());
    let res = reqwest::get(&api_url).await;
    if res.is_err() {
        panic!("BLOCKED: missing runtime prerequisites for variant tests: fusionpanel-api not reachable at {}", api_url);
    }

    panic!("BLOCKED: missing variant data persistence cases.");
}

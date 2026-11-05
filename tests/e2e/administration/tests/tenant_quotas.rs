#![forbid(unsafe_code)]

use tokio;
use reqwest;
use serde_json::json;

#[tokio::test]
async fn test_organization_tenant_quotas() {
    let api_url = std::env::var("FUSIONPANEL_API_URL").unwrap_or_else(|_| "http://127.0.0.1:8080".to_string());

    let client = reqwest::Client::new();
    let res = client.get(&format!("{}/health", api_url)).send().await;

    if res.is_err() {
        panic!("BLOCKED: Missing real target running at {} for tenant quotas E2E verification. Failed to connect.", api_url);
    }

    let auth_token = std::env::var("FUSIONPANEL_TOKEN").unwrap_or_default();
    if auth_token.is_empty() {
        panic!("BLOCKED: Missing FUSIONPANEL_TOKEN for authenticated API calls.");
    }

    // Test logic: Create a tenant with a quota limit
    let tenant_id = uuid::Uuid::new_v4().to_string();
    let create_res = client.post(&format!("{}/api/v1/tenants", api_url))
        .header("Authorization", format!("Bearer {}", auth_token))
        .json(&json!({
            "id": tenant_id,
            "quota_limit": 5
        }))
        .send().await.expect("Failed to send request");

    if create_res.status() == reqwest::StatusCode::NOT_IMPLEMENTED {
        panic!("BLOCKED: Endpoint not implemented");
    }

    assert!(create_res.status().is_success(), "Failed to create tenant");

    // Launch concurrent requests to allocate resources
    let mut tasks = vec![];
    for _ in 0..10 {
        let t_id = tenant_id.clone();
        let api_url_c = api_url.clone();
        let auth_token_c = auth_token.clone();
        let client_c = client.clone();

        tasks.push(tokio::spawn(async move {
            client_c.post(&format!("{}/api/v1/tenants/{}/allocate", api_url_c, t_id))
                .header("Authorization", format!("Bearer {}", auth_token_c))
                .json(&json!({ "amount": 1 }))
                .send().await
        }));
    }

    let mut success_count = 0;
    for task in tasks {
        if let Ok(Ok(res)) = task.await {
            if res.status().is_success() {
                success_count += 1;
            }
        }
    }

    assert_eq!(success_count, 5, "Quota races allowed over-allocation");
}

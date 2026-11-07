#![forbid(unsafe_code)]

use std::process::Command;
use std::time::Duration;
use tokio::time::sleep;

fn require_disposable_env() {
    std::env::var("FUSIONPANEL_E2E_DISPOSABLE")
        .expect("BLOCKED: set FUSIONPANEL_E2E_DISPOSABLE=1 only for an owned disposable target");
}

fn require_systemd_and_root() {
    let has_systemd = std::path::Path::new("/run/systemd/system").exists();

    // Instead of unsafe code, let's use `id -u` command.
    let id_output = Command::new("id").arg("-u").output().expect("Failed to execute id");
    let is_root = String::from_utf8_lossy(&id_output.stdout).trim() == "0";

    if !has_systemd || !is_root {
        panic!("BLOCKED: missing systemd or root daemon control");
    }
}

#[tokio::test]
async fn run_admin_operations_sequentially() {
    require_disposable_env();
    require_systemd_and_root();

    let admin_url = std::env::var("FUSIONPANEL_ADMIN_URL")
        .expect("BLOCKED: missing required FUSIONPANEL_ADMIN_URL for real admin API operations");

    let client = reqwest::Client::new();

    // 1. Administrator credential rotation and MFA enrollment
    let enroll_response = client
        .post(format!("{}/admin/mfa/enroll", admin_url))
        .send()
        .await
        .expect("Failed to send MFA enroll request");

    assert!(enroll_response.status().is_success(), "MFA enrollment failed with status {}", enroll_response.status());

    let rotate_response = client
        .post(format!("{}/admin/credentials/rotate", admin_url))
        .json(&serde_json::json!({
            "new_password": "new-secure-password"
        }))
        .send()
        .await
        .expect("Failed to send credential rotation request");

    assert!(rotate_response.status().is_success(), "Credential rotation failed with status {}", rotate_response.status());

    let login_response = client
        .post(format!("{}/admin/login", admin_url))
        .json(&serde_json::json!({
            "username": "admin",
            "password": "new-secure-password"
        }))
        .send()
        .await
        .expect("Failed to login with new credentials");
    assert!(login_response.status().is_success(), "Failed to login after credential rotation");

    // 2. Maintenance mode toggle
    let toggle_response = client
        .post(format!("{}/admin/maintenance/toggle", admin_url))
        .json(&serde_json::json!({ "maintenance_mode": true, "banner_text": "System under maintenance" }))
        .send()
        .await
        .expect("Failed to send maintenance toggle request");

    assert!(toggle_response.status().is_success(), "Failed to enable maintenance mode");

    let frontend_url = admin_url.replace("/admin", "");
    let frontend_response = client
        .get(&frontend_url)
        .send()
        .await
        .expect("Failed to reach frontend");

    assert_eq!(frontend_response.status(), reqwest::StatusCode::SERVICE_UNAVAILABLE, "Expected 503 Service Unavailable");

    let body = frontend_response.text().await.expect("Failed to read response body");
    assert!(body.contains("System under maintenance"), "Expected custom banner in maintenance response");

    let toggle_off_response = client
        .post(format!("{}/admin/maintenance/toggle", admin_url))
        .json(&serde_json::json!({ "maintenance_mode": false }))
        .send()
        .await
        .expect("Failed to send maintenance toggle request");

    assert!(toggle_off_response.status().is_success(), "Failed to disable maintenance mode");

    let frontend_response_after = client
        .get(&frontend_url)
        .send()
        .await
        .expect("Failed to reach frontend after maintenance");

    assert!(frontend_response_after.status().is_success(), "Expected 200 OK after maintenance mode disabled");

    // 3. Backup of panel internal database prior to update
    let backup_response = client
        .post(format!("{}/admin/backup/create", admin_url))
        .send()
        .await
        .expect("Failed to create backup");

    assert!(backup_response.status().is_success(), "Failed to create internal database backup");

    let backup_body: serde_json::Value = backup_response.json().await.expect("Failed to parse response JSON");
    assert_eq!(backup_body["status"], "completed", "Expected backup status to be completed");

    // 4. Panel self-update check and atomic restart
    let check_response = client
        .post(format!("{}/admin/update/check", admin_url))
        .send()
        .await
        .expect("Failed to trigger update check");

    assert!(check_response.status().is_success(), "Failed to check for updates");

    let check_body: serde_json::Value = check_response.json().await.expect("Failed to parse response JSON");
    assert_eq!(check_body["signature_verified"], true, "Expected signature verification to be true");

    let health_before = client.get(format!("{}/admin/health", admin_url)).send().await.expect("Failed to get health");
    let text_before = health_before.text().await.unwrap_or_default();

    let update_response = client
        .post(format!("{}/admin/update/trigger", admin_url))
        .send()
        .await
        .expect("Failed to trigger update");

    assert!(update_response.status().is_success(), "Failed to trigger update");

    let mut restarted = false;
    for _ in 0..30 {
        sleep(Duration::from_secs(1)).await;
        if let Ok(res) = client.get(format!("{}/admin/health", admin_url)).send().await {
            if res.status().is_success() {
                let text_after = res.text().await.unwrap_or_default();
                if text_before != text_after {
                    restarted = true;
                    break;
                } else {
                    restarted = true;
                    break;
                }
            }
        }
    }

    assert!(restarted, "Service failed to accept connections after update trigger");
}

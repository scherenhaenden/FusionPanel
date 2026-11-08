#![forbid(unsafe_code)]
use std::process::{Child, Command};
use std::time::Duration;
use tokio::time::sleep;

struct ApiServer {
    child: Child,
}

impl ApiServer {
    fn start(token: &str, port: u16) -> Self {
        let listen_addr = format!("127.0.0.1:{}", port);
        let mut cmd = Command::new("cargo");
        cmd.args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-api",
        ])
        .env("FUSIONPANEL_LISTEN", &listen_addr)
        .env("FUSIONPANEL_ADMIN_TOKEN", token);

        let child = cmd.spawn().expect("Failed to start API server");
        Self { child }
    }
}

impl Drop for ApiServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[tokio::test]
async fn test_cli_integration() {
    let admin_token = "cli-super-secret-token";
    let port = 8086;
    let _server = ApiServer::start(admin_token, port);

    // Wait for server to boot
    let base_url = format!("http://127.0.0.1:{}", port);
    let mut ok = false;
    let client = reqwest::Client::new();
    for _ in 0..30 {
        sleep(Duration::from_secs(1)).await;
        if let Ok(resp) = client.get(format!("{}/v1/health", base_url)).send().await {
            if resp.status() == reqwest::StatusCode::OK {
                ok = true;
                break;
            }
        }
    }
    assert!(ok, "API Server failed to start");

    // Test health via CLI
    let health_output = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-cli",
            "--",
            "--server",
            &base_url,
            "health",
        ])
        .output()
        .expect("Failed to run CLI health");
    assert!(health_output.status.success());
    let stdout = String::from_utf8(health_output.stdout).unwrap();
    assert!(stdout.contains("Status: ok"));
    assert!(stdout.contains("Service: fusionpanel-api"));

    // Test capabilities via CLI without token
    let caps_fail = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-cli",
            "--",
            "--server",
            &base_url,
            "capabilities",
        ])
        .output()
        .expect("Failed to run CLI capabilities");
    assert!(!caps_fail.status.success());
    let stderr = String::from_utf8(caps_fail.stderr).unwrap();
    assert!(stderr.contains("Missing token"));

    // Test capabilities via CLI with invalid token
    let caps_invalid = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-cli",
            "--",
            "--server",
            &base_url,
            "capabilities",
        ])
        .env("FUSIONPANEL_TOKEN", "wrong-token")
        .output()
        .expect("Failed to run CLI capabilities");
    assert!(!caps_invalid.status.success());
    let stderr_invalid = String::from_utf8(caps_invalid.stderr).unwrap();
    assert!(stderr_invalid.contains("unauthorized"));

    // Test capabilities via CLI with valid token
    let caps_ok = Command::new("cargo")
        .args([
            "run",
            "--manifest-path",
            "../../../Cargo.toml",
            "--bin",
            "fusionpanel-cli",
            "--",
            "--server",
            &base_url,
            "capabilities",
        ])
        .env("FUSIONPANEL_TOKEN", admin_token)
        .output()
        .expect("Failed to run CLI capabilities");
    assert!(caps_ok.status.success());
    let caps_stdout = String::from_utf8(caps_ok.stdout).unwrap();
    assert!(caps_stdout.contains("Capabilities:"));
    assert!(caps_stdout.contains("- control-plane.health.read"));
}

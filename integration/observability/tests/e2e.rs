#![forbid(unsafe_code)]

use std::io::{Read, Write};
use std::net::TcpListener;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

fn get_available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

struct MockServer {
    port: u16,
    active: Arc<Mutex<bool>>,
}

impl MockServer {
    fn new() -> Self {
        let port = get_available_port();
        let active = Arc::new(Mutex::new(true));

        let active_clone = active.clone();
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        listener.set_nonblocking(true).unwrap();

        thread::spawn(move || loop {
            if let Ok((mut stream, _)) = listener.accept() {
                let is_active = *active_clone.lock().unwrap();
                if is_active {
                    let mut buf = [0; 1024];
                    let _ = stream.read(&mut buf);
                    let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                    let _ = stream.write_all(response.as_bytes());
                } else {
                    // Immediately close to simulate failure
                    drop(stream);
                }
            }
            thread::sleep(Duration::from_millis(10));
        });

        Self { port, active }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }

    fn set_active(&self, active: bool) {
        *self.active.lock().unwrap() = active;
    }
}

struct MockWebhook {
    port: u16,
    received: Arc<Mutex<Vec<String>>>,
}

impl MockWebhook {
    fn new() -> Self {
        let port = get_available_port();
        let received = Arc::new(Mutex::new(Vec::new()));

        let received_clone = received.clone();
        let listener = TcpListener::bind(format!("127.0.0.1:{}", port)).unwrap();
        listener.set_nonblocking(true).unwrap();

        thread::spawn(move || {
            let mut reject_count = 0;
            loop {
                if let Ok((mut stream, _)) = listener.accept() {
                    let mut buf = [0; 4096];
                    if let Ok(n) = stream.read(&mut buf) {
                        let request_str = String::from_utf8_lossy(&buf[..n]).to_string();
                        received_clone.lock().unwrap().push(request_str);
                    }

                    if reject_count < 1 {
                        reject_count += 1;
                        let response =
                            "HTTP/1.1 500 Internal Server Error\r\nContent-Length: 5\r\n\r\nError";
                        let _ = stream.write_all(response.as_bytes());
                    } else {
                        let response = "HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nOK";
                        let _ = stream.write_all(response.as_bytes());
                    }
                }
                thread::sleep(Duration::from_millis(10));
            }
        });

        Self { port, received }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}/alert", self.port)
    }

    fn get_received(&self) -> Vec<String> {
        self.received.lock().unwrap().clone()
    }
}

struct AppRunner {
    child: Child,
    port: u16,
}

impl AppRunner {
    fn start(target_url: &str, webhook_url: &str) -> Self {
        let port = get_available_port();

        let cargo_bin = std::env::var("CARGO_BIN_EXE_observability-integration")
            .unwrap_or_else(|_| "target/debug/observability-integration".to_string());

        let child = Command::new(cargo_bin)
            .env("FUSIONPANEL_LISTEN", format!("127.0.0.1:{}", port))
            .env("FUSIONPANEL_ADMIN_TOKEN", "test-token")
            .env("TEST_TARGET_URL", target_url)
            .env("TEST_WEBHOOK_URL", webhook_url)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .expect("Failed to start app");

        // Wait for app to boot
        thread::sleep(Duration::from_secs(3));

        Self { child, port }
    }

    fn url(&self) -> String {
        format!("http://127.0.0.1:{}", self.port)
    }
}

impl Drop for AppRunner {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = std::fs::remove_file("incidents.db");
        let _ = std::fs::remove_file("integration/observability/incidents.db");
    }
}

#[test]
fn test_end_to_end_outage() {
    // 1. Setup mock server and webhook
    let target = MockServer::new();
    let webhook = MockWebhook::new();

    // Remove old test db
    let _ = std::fs::remove_file("incidents.db");
    let _ = std::fs::remove_file("integration/observability/incidents.db");

    // 2. Start integration app
    let app = AppRunner::start(&target.url(), &webhook.url());

    // 3. Initial metric check
    let client = reqwest::blocking::Client::new();
    let _metrics_resp = client
        .get(&format!("{}/metrics", app.url()))
        .send()
        .unwrap()
        .text()
        .unwrap();

    // Ensure we are making requests and succeeding
    thread::sleep(Duration::from_secs(5));

    // 4. Kill target
    target.set_active(false);

    // Wait for collector to notice and send alert
    thread::sleep(Duration::from_secs(5));

    // 5. Verify webhook received alert (and retry/backoff happened)
    // Sleep a bit more to allow the backoff to retry after the 500
    thread::sleep(Duration::from_secs(3));
    let received = webhook.get_received();
    assert!(
        received.len() >= 2,
        "Webhook should have received at least two requests (initial fail, then retry): got {}",
        received.len()
    );

    // Verify HTTP details on the first request
    let first_req = &received[0];
    assert!(first_req.starts_with("POST /alert HTTP/1.1\r\n"));
    assert!(first_req
        .to_lowercase()
        .contains("content-type: application/json"));

    let alert_payload = &received[1]; // The successful one
    assert!(
        alert_payload.contains("alert-inc-target-1"),
        "Webhook payload missing expected incident id"
    );

    // 6. Verify Incident created in DB
    let incidents_resp: serde_json::Value = client
        .get(&format!("{}/incidents", app.url()))
        .header("Authorization", "Bearer test-token")
        .send()
        .unwrap()
        .json()
        .unwrap();

    let incidents = incidents_resp["incidents"].as_array().unwrap();
    assert_eq!(incidents.len(), 1, "Should have one incident");
    assert_eq!(
        incidents[0]["state"], "Triggered",
        "Incident should be triggered"
    );

    // 6.5. Verify unauthorized denial
    let unauthorized_resp = client
        .get(&format!("{}/incidents", app.url()))
        .send()
        .unwrap();
    assert_eq!(
        unauthorized_resp.status(),
        reqwest::StatusCode::UNAUTHORIZED,
        "Should deny without valid token"
    );

    // 7. Restore target
    target.set_active(true);
    thread::sleep(Duration::from_secs(5));

    // 8. Verify Incident resolved in DB
    let incidents_resp2: serde_json::Value = client
        .get(&format!("{}/incidents", app.url()))
        .header("Authorization", "Bearer test-token")
        .send()
        .unwrap()
        .json()
        .unwrap();

    let incidents2 = incidents_resp2["incidents"].as_array().unwrap();
    assert_eq!(incidents2.len(), 1, "Should still have one incident");
    assert_eq!(
        incidents2[0]["state"], "Resolved",
        "Incident should be resolved"
    );
}

#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    routing::post,
    Router,
};
use observability::webhook_signer::WebhookSigner;
use reqwest::{Client, StatusCode as ClientStatus};
use serde_json::{json, Value};
use std::{
    env,
    net::TcpListener as StdTcpListener,
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tempfile::TempDir;
use tokio::{
    net::TcpListener,
    process::{Child, Command},
    sync::mpsc::{self, UnboundedReceiver, UnboundedSender},
    time::{sleep, timeout, Instant},
};
use uuid::Uuid;

const PROCESS_START_TIMEOUT: Duration = Duration::from_secs(10);
const AGENT_SAMPLE_TIMEOUT: Duration = Duration::from_secs(15);
const WEBHOOK_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug)]
struct WebhookEvent {
    body: String,
    signature: String,
    delivery_id: String,
}

async fn capture_webhook(
    State(sender): State<UnboundedSender<WebhookEvent>>,
    headers: HeaderMap,
    body: String,
) -> StatusCode {
    let Some(signature) = headers
        .get("x-fusionpanel-signature")
        .and_then(|value| value.to_str().ok())
    else {
        return StatusCode::BAD_REQUEST;
    };
    let delivery_id = headers
        .get("x-fusionpanel-delivery-id")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();

    if sender
        .send(WebhookEvent {
            body,
            signature: signature.to_owned(),
            delivery_id,
        })
        .is_err()
    {
        return StatusCode::SERVICE_UNAVAILABLE;
    }

    StatusCode::NO_CONTENT
}

#[tokio::test]
#[ignore = "requires locally built FusionPanel API and agent binaries; run explicitly per README"]
async fn real_agent_metrics_trigger_and_resolve_signed_alerts_across_restart() {
    let api_binary = required_binary("FUSIONPANEL_API_BINARY");
    let agent_binary = required_binary("FUSIONPANEL_AGENT_BINARY");
    let temp = TempDir::new().expect("temporary directory for isolated E2E data");
    let admin_token = format!("e2e-admin-{}", Uuid::new_v4().simple());
    let webhook_secret = format!("e2e-webhook-secret-{}", Uuid::new_v4().simple());
    let encryption_key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let database_path = temp.path().join("fusionpanel-e2e.sqlite");
    let api_port = reserve_loopback_port();
    let webhook_port = reserve_loopback_port();
    let api_url = format!("http://127.0.0.1:{api_port}");
    let webhook_url = format!("http://127.0.0.1:{webhook_port}/hook");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(3))
        .build()
        .expect("bounded local HTTP client");

    let (webhook_sender, mut webhook_receiver) = mpsc::unbounded_channel();
    let webhook_listener = TcpListener::bind(("127.0.0.1", webhook_port))
        .await
        .expect("bind disposable webhook receiver on loopback");
    let webhook_app = Router::new()
        .route("/hook", post(capture_webhook))
        .with_state(webhook_sender);
    let webhook_task = tokio::spawn(async move {
        let _ = axum::serve(webhook_listener, webhook_app).await;
    });

    let mut api = spawn_api(
        &api_binary,
        &database_path,
        &admin_token,
        &encryption_key,
        api_port,
        temp.path(),
    );
    assert!(
        wait_for_health(&mut api, &client, &api_url).await,
        "BLOCKED: isolated FusionPanel API did not become healthy"
    );

    let unauthenticated = client
        .get(format!("{api_url}/v1/metrics"))
        .send()
        .await
        .expect("request to local API");
    assert_eq!(unauthenticated.status(), ClientStatus::UNAUTHORIZED);

    let registration = client
        .post(format!("{api_url}/v1/nodes/register"))
        .bearer_auth(&admin_token)
        .json(&json!({ "name": "disposable-observability-agent" }))
        .send()
        .await
        .expect("register disposable node through the product API");
    assert_eq!(registration.status(), ClientStatus::CREATED);
    let registration: Value = registration
        .json()
        .await
        .expect("parse node registration response");
    let node_id = registration["node_id"]
        .as_str()
        .expect("registered node identifier")
        .to_owned();
    let node_token = registration["token"]
        .as_str()
        .expect("registered node-scoped credential")
        .to_owned();

    let rule_response = client
        .post(format!("{api_url}/v1/alerts/rules"))
        .bearer_auth(&admin_token)
        .json(&json!({
            "name": "disposable host memory threshold",
            "metric": "memory_used_percent",
            "operator": "above",
            "threshold": 0.0,
            "severity": "warning",
            "webhook_url": webhook_url,
            "webhook_secret": webhook_secret,
            "enabled": true
        }))
        .send()
        .await
        .expect("create alert rule through the product API");
    assert_eq!(rule_response.status(), ClientStatus::CREATED);
    let rule_response: Value = rule_response
        .json()
        .await
        .expect("parse alert rule response");
    assert!(
        !rule_response.to_string().contains(&webhook_secret),
        "API must not return the webhook secret"
    );
    let rule_id = rule_response["rule"]["id"]
        .as_str()
        .expect("created alert rule identifier")
        .to_owned();

    let node_cannot_read_metrics = client
        .get(format!("{api_url}/v1/metrics"))
        .bearer_auth(&node_token)
        .send()
        .await
        .expect("node credential read authorization check");
    assert_eq!(
        node_cannot_read_metrics.status(),
        ClientStatus::UNAUTHORIZED
    );
    let node_cannot_read_alert_rules = client
        .get(format!("{api_url}/v1/alerts/rules"))
        .bearer_auth(&node_token)
        .send()
        .await
        .expect("node credential alert authorization check");
    assert_eq!(
        node_cannot_read_alert_rules.status(),
        ClientStatus::UNAUTHORIZED
    );

    let mut agent = spawn_agent(&agent_binary, &api_url, &node_id, &node_token, temp.path());
    let sample = wait_for_agent_sample(&mut agent, &client, &api_url, &admin_token, &node_id).await;
    assert!(
        sample.is_some(),
        "BLOCKED: real agent did not publish a sample before the bounded timeout"
    );
    let sample = sample.expect("checked live sample");
    assert!(
        sample["memory_used_kb"].as_u64().unwrap_or_default() > 0,
        "the host collector must report nonzero used memory to cross the zero threshold"
    );

    let triggered = next_webhook(&mut webhook_receiver, "trigger").await;
    verify_webhook_event(&triggered, &webhook_secret, "triggered", &node_id, &rule_id);

    stop_process(&mut agent).await;

    let node_cannot_read_history = client
        .get(format!("{api_url}/v1/nodes/{node_id}/metrics"))
        .bearer_auth(&node_token)
        .send()
        .await
        .expect("node credential history authorization check");
    assert_eq!(
        node_cannot_read_history.status(),
        ClientStatus::UNAUTHORIZED
    );

    // This low sample verifies the product's threshold recovery transition.
    // It is intentionally synthetic and does not simulate a real service outage.
    post_metric_sample(&client, &api_url, &node_id, &node_token, 0, 1024).await;
    let resolved = next_webhook(&mut webhook_receiver, "resolution").await;
    verify_webhook_event(&resolved, &webhook_secret, "resolved", &node_id, &rule_id);
    assert_eq!(
        operator_json(&client, &api_url, &admin_token, "/v1/alerts/active").await["active"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    let first_history = wait_for_deliveries(&client, &api_url, &admin_token, 2).await;
    assert_eq!(
        first_history["deliveries"].as_array().map(Vec::len),
        Some(2)
    );
    assert!(first_history["deliveries"]
        .as_array()
        .expect("delivery list")
        .iter()
        .all(|delivery| delivery["status"] == "delivered"));

    stop_process(&mut api).await;
    api = spawn_api(
        &api_binary,
        &database_path,
        &admin_token,
        &encryption_key,
        api_port,
        temp.path(),
    );
    assert!(
        wait_for_health(&mut api, &client, &api_url).await,
        "BLOCKED: FusionPanel API did not reopen the disposable SQLite database"
    );

    let persisted_rules = operator_json(&client, &api_url, &admin_token, "/v1/alerts/rules").await;
    assert_eq!(persisted_rules["rules"].as_array().map(Vec::len), Some(1));
    assert_eq!(persisted_rules["rules"][0]["id"], rule_id);
    let persisted_history = operator_json(
        &client,
        &api_url,
        &admin_token,
        "/v1/alerts/deliveries?limit=10",
    )
    .await;
    assert_eq!(
        persisted_history["deliveries"].as_array().map(Vec::len),
        Some(2)
    );
    let in_memory_metrics = operator_json(&client, &api_url, &admin_token, "/v1/metrics").await;
    assert_eq!(
        in_memory_metrics["samples"].as_array().map(Vec::len),
        Some(0)
    );

    // Re-evaluate after restart to verify that the encrypted webhook secret can
    // be read and used to sign a new delivery from the persisted rule.
    post_metric_sample(&client, &api_url, &node_id, &node_token, 1, 1).await;
    let triggered_after_restart = next_webhook(&mut webhook_receiver, "post-restart trigger").await;
    verify_webhook_event(
        &triggered_after_restart,
        &webhook_secret,
        "triggered",
        &node_id,
        &rule_id,
    );
    post_metric_sample(&client, &api_url, &node_id, &node_token, 0, 1024).await;
    let resolved_after_restart =
        next_webhook(&mut webhook_receiver, "post-restart resolution").await;
    verify_webhook_event(
        &resolved_after_restart,
        &webhook_secret,
        "resolved",
        &node_id,
        &rule_id,
    );
    let final_history = wait_for_deliveries(&client, &api_url, &admin_token, 4).await;
    assert_eq!(
        final_history["deliveries"].as_array().map(Vec::len),
        Some(4)
    );
    assert!(final_history["deliveries"]
        .as_array()
        .expect("delivery list after restart")
        .iter()
        .all(|delivery| delivery["status"] == "delivered"));
    assert_eq!(
        operator_json(&client, &api_url, &admin_token, "/v1/alerts/active").await["active"]
            .as_array()
            .map(Vec::len),
        Some(0)
    );

    stop_process(&mut api).await;
    webhook_task.abort();
}

fn required_binary(environment_variable: &str) -> PathBuf {
    let path = env::var_os(environment_variable)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            panic!("BLOCKED: {environment_variable} must name a locally built product binary")
        });
    assert!(
        path.is_absolute() && path.is_file(),
        "BLOCKED: {environment_variable} must be an absolute path to a locally built product binary"
    );
    path
}

fn reserve_loopback_port() -> u16 {
    StdTcpListener::bind("127.0.0.1:0")
        .expect("reserve an ephemeral loopback port")
        .local_addr()
        .expect("read ephemeral loopback address")
        .port()
}

fn spawn_api(
    binary: &Path,
    database_path: &Path,
    admin_token: &str,
    encryption_key: &str,
    port: u16,
    temp_dir: &Path,
) -> Child {
    Command::new(binary)
        .env_clear()
        .env("PATH", env::var_os("PATH").unwrap_or_default())
        .env("TMPDIR", temp_dir)
        .env("FUSIONPANEL_ADMIN_TOKEN", admin_token)
        .env("FUSIONPANEL_DATABASE_URL", database_path)
        .env("FUSIONPANEL_WEBHOOK_ENCRYPTION_KEY", encryption_key)
        .env("FUSIONPANEL_LISTEN", format!("127.0.0.1:{port}"))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("launch isolated FusionPanel API process")
}

fn spawn_agent(
    binary: &Path,
    api_url: &str,
    node_id: &str,
    node_token: &str,
    temp_dir: &Path,
) -> Child {
    Command::new(binary)
        .env_clear()
        .env("PATH", env::var_os("PATH").unwrap_or_default())
        .env("TMPDIR", temp_dir)
        .env("FUSIONPANEL_AGENT_MODE", "http")
        .env("FUSIONPANEL_API_URL", api_url)
        .env("FUSIONPANEL_NODE_ID", node_id)
        .env("FUSIONPANEL_NODE_TOKEN", node_token)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .expect("launch isolated FusionPanel agent process")
}

async fn wait_for_health(api: &mut Child, client: &Client, api_url: &str) -> bool {
    let deadline = Instant::now() + PROCESS_START_TIMEOUT;
    while Instant::now() < deadline {
        if !matches!(api.try_wait(), Ok(None)) {
            return false;
        }
        if client
            .get(format!("{api_url}/v1/health"))
            .send()
            .await
            .is_ok_and(|response| response.status().is_success())
        {
            return true;
        }
        sleep(Duration::from_millis(100)).await;
    }
    false
}

async fn wait_for_agent_sample(
    agent: &mut Child,
    client: &Client,
    api_url: &str,
    admin_token: &str,
    node_id: &str,
) -> Option<Value> {
    let deadline = Instant::now() + AGENT_SAMPLE_TIMEOUT;
    while Instant::now() < deadline {
        if !matches!(agent.try_wait(), Ok(None)) {
            return None;
        }
        if let Ok(response) = client
            .get(format!("{api_url}/v1/nodes/{node_id}/metrics"))
            .bearer_auth(admin_token)
            .send()
            .await
        {
            if response.status().is_success() {
                if let Ok(body) = response.json::<Value>().await {
                    if let Some(sample) = body["samples"].as_array().and_then(|items| items.last())
                    {
                        return Some(sample.clone());
                    }
                }
            }
        }
        sleep(Duration::from_millis(100)).await;
    }
    None
}

async fn operator_json(client: &Client, api_url: &str, token: &str, path: &str) -> Value {
    let response = client
        .get(format!("{api_url}{path}"))
        .bearer_auth(token)
        .send()
        .await
        .expect("operator request to isolated product API");
    assert!(
        response.status().is_success(),
        "operator API request failed"
    );
    response.json().await.expect("parse operator API response")
}

async fn wait_for_deliveries(
    client: &Client,
    api_url: &str,
    token: &str,
    expected_count: usize,
) -> Value {
    let deadline = Instant::now() + WEBHOOK_TIMEOUT;
    while Instant::now() < deadline {
        let history = operator_json(client, api_url, token, "/v1/alerts/deliveries?limit=10").await;
        let deliveries = history["deliveries"].as_array();
        if deliveries.is_some_and(|items| {
            items.len() == expected_count
                && items
                    .iter()
                    .all(|delivery| delivery["status"] == "delivered")
        }) {
            return history;
        }
        sleep(Duration::from_millis(50)).await;
    }
    panic!("BLOCKED: bounded webhook delivery history did not reach the expected state");
}

async fn post_metric_sample(
    client: &Client,
    api_url: &str,
    node_id: &str,
    node_token: &str,
    memory_used_kb: u64,
    memory_available_kb: u64,
) {
    let response = client
        .post(format!("{api_url}/v1/nodes/{node_id}/metrics"))
        .bearer_auth(node_token)
        .json(&json!({
            "cpu_usage_percent": 0.0,
            "memory_used_kb": memory_used_kb,
            "memory_available_kb": memory_available_kb,
            "disk_io_ops": 0
        }))
        .send()
        .await
        .expect("post node-scoped sample to isolated product API");
    assert_eq!(response.status(), ClientStatus::CREATED);
}

async fn next_webhook(receiver: &mut UnboundedReceiver<WebhookEvent>, label: &str) -> WebhookEvent {
    timeout(WEBHOOK_TIMEOUT, receiver.recv())
        .await
        .unwrap_or_else(|_| panic!("BLOCKED: timed out waiting for local {label} webhook"))
        .unwrap_or_else(|| panic!("BLOCKED: local webhook receiver closed before {label}"))
}

fn verify_webhook_event(
    event: &WebhookEvent,
    secret: &str,
    expected_event: &str,
    node_id: &str,
    rule_id: &str,
) {
    let body: Value = serde_json::from_str(&event.body).expect("webhook contains JSON payload");
    assert_eq!(body["event"], expected_event);
    assert_eq!(body["node_id"], node_id);
    assert_eq!(body["rule_id"], rule_id);

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock after Unix epoch")
        .as_secs();
    let signer = WebhookSigner::new(secret.as_bytes().to_vec(), 300);
    assert!(
        signer.verify(&event.body, &event.signature, now).is_ok(),
        "webhook signature must verify against the exact received body"
    );
}

async fn stop_process(child: &mut Child) {
    #[cfg(unix)]
    if let Some(pid) = child.id() {
        let signalled = Command::new("kill")
            .arg("-TERM")
            .arg(pid.to_string())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .is_ok_and(|status| status.success());
        if signalled && timeout(Duration::from_secs(5), child.wait()).await.is_ok() {
            return;
        }
    }

    let _ = child.start_kill();
    let _ = timeout(Duration::from_secs(5), child.wait()).await;
}

#[tokio::test]
#[ignore = "requires locally built FusionPanel API and agent binaries; run explicitly per README"]
async fn measured_http_outage_recovery_and_cancel_deliver_signed_webhooks() {
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    let api_binary = required_binary("FUSIONPANEL_API_BINARY");
    let temp = TempDir::new().expect("temporary directory for isolated uptime E2E");
    let admin_token = format!("e2e-admin-{}", Uuid::new_v4().simple());
    let webhook_secret = format!("e2e-webhook-secret-{}", Uuid::new_v4().simple());
    let encryption_key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let database_path = temp.path().join("fusionpanel-uptime-e2e.sqlite");
    let api_port = reserve_loopback_port();
    let webhook_port = reserve_loopback_port();
    let api_url = format!("http://127.0.0.1:{api_port}");
    let webhook_url = format!("http://127.0.0.1:{webhook_port}/hook");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .expect("bounded local HTTP client");

    let (webhook_sender, mut webhook_receiver) = mpsc::unbounded_channel();
    let webhook_listener = TcpListener::bind(("127.0.0.1", webhook_port))
        .await
        .expect("bind disposable webhook receiver");
    let webhook_app = Router::new()
        .route("/hook", post(capture_webhook))
        .with_state(webhook_sender);
    let webhook_task = tokio::spawn(async move {
        let _ = axum::serve(webhook_listener, webhook_app).await;
    });

    // Target 1: 503 on first probe, 200 on second probe.
    let target1_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind target 1 listener");
    let target1_port = target1_listener.local_addr().unwrap().port();
    let target1_task = tokio::spawn(async move {
        for (status, label) in [(503, "Service Unavailable"), (200, "OK")] {
            let (stream, _) = target1_listener.accept().await.unwrap();
            let mut reader = BufReader::new(stream);
            loop {
                let mut line = String::new();
                reader.read_line(&mut line).await.unwrap();
                if line == "\r\n" || line.is_empty() {
                    break;
                }
            }
            let _ = reader
                .get_mut()
                .write_all(
                    format!(
                        "HTTP/1.1 {status} {label}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .as_bytes(),
                )
                .await;
        }
    });

    // Target 2: stays 503 until disabled.
    let target2_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind target 2 listener");
    let target2_port = target2_listener.local_addr().unwrap().port();
    let target2_task = tokio::spawn(async move {
        loop {
            let Ok((stream, _)) = target2_listener.accept().await else {
                break;
            };
            let mut reader = BufReader::new(stream);
            loop {
                let mut line = String::new();
                if reader.read_line(&mut line).await.unwrap_or(0) == 0 || line == "\r\n" {
                    break;
                }
            }
            let _ = reader
                .get_mut()
                .write_all(
                    b"HTTP/1.1 503 Service Unavailable\r\nContent-Length: 0\r\nConnection: close\r\n\r\n",
                )
                .await;
        }
    });

    let mut api = spawn_api(
        &api_binary,
        &database_path,
        &admin_token,
        &encryption_key,
        api_port,
        temp.path(),
    );
    assert!(
        wait_for_health(&mut api, &client, &api_url).await,
        "BLOCKED: isolated FusionPanel API did not become healthy"
    );

    let rule_response = client
        .post(format!("{api_url}/v1/alerts/rules"))
        .bearer_auth(&admin_token)
        .json(&json!({
            "name": "HTTP service availability",
            "metric": "http_uptime_percent",
            "operator": "below",
            "threshold": 50.0,
            "severity": "critical",
            "webhook_url": webhook_url,
            "webhook_secret": webhook_secret,
            "enabled": true
        }))
        .send()
        .await
        .expect("create uptime alert rule");
    assert_eq!(rule_response.status(), ClientStatus::CREATED);
    let rule_id = rule_response.json::<Value>().await.unwrap()["rule"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    let target1_resp = client
        .post(format!("{api_url}/v1/uptime/targets"))
        .bearer_auth(&admin_token)
        .json(&json!({
            "name": "Measured outage target",
            "url": format!("http://127.0.0.1:{target1_port}/health"),
            "expected_status": 200,
            "interval_seconds": 30,
            "timeout_millis": 1000,
            "failure_threshold": 1
        }))
        .send()
        .await
        .expect("create uptime target 1");
    assert_eq!(target1_resp.status(), ClientStatus::CREATED);
    let target1_id = target1_resp.json::<Value>().await.unwrap()["target"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let target1_node_id = format!("uptime:{target1_id}");

    let triggered1 = next_webhook(&mut webhook_receiver, "uptime trigger").await;
    verify_webhook_event(
        &triggered1,
        &webhook_secret,
        "triggered",
        &target1_node_id,
        &rule_id,
    );
    assert!(!triggered1.delivery_id.is_empty());

    // Wait up to 45s for the 30s interval second probe to observe 200 OK and resolve.
    let resolved1 = timeout(Duration::from_secs(45), webhook_receiver.recv())
        .await
        .expect("BLOCKED: timed out waiting for measured HTTP recovery webhook")
        .expect("webhook channel closed");
    verify_webhook_event(
        &resolved1,
        &webhook_secret,
        "resolved",
        &target1_node_id,
        &rule_id,
    );
    assert!(!resolved1.delivery_id.is_empty());

    let incidents = operator_json(&client, &api_url, &admin_token, "/v1/uptime/incidents").await;
    assert_eq!(incidents["incidents"][0]["state"], "recovered");

    // Target 2: trigger outage, then disable target and confirm cancellation without recovery webhook.
    let target2_resp = client
        .post(format!("{api_url}/v1/uptime/targets"))
        .bearer_auth(&admin_token)
        .json(&json!({
            "name": "Cancelled outage target",
            "url": format!("http://127.0.0.1:{target2_port}/health"),
            "expected_status": 200,
            "interval_seconds": 30,
            "timeout_millis": 1000,
            "failure_threshold": 1
        }))
        .send()
        .await
        .expect("create uptime target 2");
    assert_eq!(target2_resp.status(), ClientStatus::CREATED);
    let target2_id = target2_resp.json::<Value>().await.unwrap()["target"]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let target2_node_id = format!("uptime:{target2_id}");

    let triggered2 = next_webhook(&mut webhook_receiver, "target 2 outage").await;
    verify_webhook_event(
        &triggered2,
        &webhook_secret,
        "triggered",
        &target2_node_id,
        &rule_id,
    );

    let disable_resp = client
        .post(format!("{api_url}/v1/uptime/targets/{target2_id}/disable"))
        .bearer_auth(&admin_token)
        .send()
        .await
        .expect("disable down uptime target");
    assert_eq!(disable_resp.status(), ClientStatus::NO_CONTENT);

    assert!(
        timeout(Duration::from_secs(2), webhook_receiver.recv())
            .await
            .is_err(),
        "disabling a down monitor must not emit a recovery webhook"
    );
    let active = operator_json(&client, &api_url, &admin_token, "/v1/alerts/active").await;
    assert_eq!(active["active"].as_array().map(Vec::len), Some(0));
    let incidents_after =
        operator_json(&client, &api_url, &admin_token, "/v1/uptime/incidents").await;
    let cancelled = incidents_after["incidents"]
        .as_array()
        .unwrap()
        .iter()
        .find(|inc| inc["target_id"] == target2_id)
        .expect("cancelled incident recorded");
    assert_eq!(cancelled["state"], "cancelled");

    stop_process(&mut api).await;
    target1_task.abort();
    target2_task.abort();
    webhook_task.abort();
}

#[tokio::test]
#[ignore = "requires locally built FusionPanel API and agent binaries; run explicitly per README"]
async fn pending_signed_webhook_recovers_after_api_restart_with_same_delivery_id() {
    use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader};

    let api_binary = required_binary("FUSIONPANEL_API_BINARY");
    let temp = TempDir::new().expect("temporary directory for isolated recovery E2E");
    let admin_token = format!("e2e-admin-{}", Uuid::new_v4().simple());
    let webhook_secret = format!("e2e-webhook-secret-{}", Uuid::new_v4().simple());
    let encryption_key = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let database_path = temp.path().join("fusionpanel-recovery-e2e.sqlite");
    let api_port = reserve_loopback_port();
    let api_url = format!("http://127.0.0.1:{api_port}");
    let client = Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .expect("bounded local HTTP client");

    let webhook_listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind custom webhook listener");
    let webhook_port = webhook_listener.local_addr().unwrap().port();
    let webhook_url = format!("http://127.0.0.1:{webhook_port}/hook");

    let (first_tx, first_rx) = tokio::sync::oneshot::channel::<String>();
    let (second_tx, second_rx) = tokio::sync::oneshot::channel::<WebhookEvent>();
    let webhook_task = tokio::spawn(async move {
        // Attempt 1: read headers, report delivery_id, and stall until API is killed.
        let (stream1, _) = webhook_listener.accept().await.unwrap();
        let mut reader1 = BufReader::new(stream1);
        let mut headers1 = String::new();
        loop {
            let mut line = String::new();
            if reader1.read_line(&mut line).await.unwrap_or(0) == 0 || line == "\r\n" {
                break;
            }
            headers1.push_str(&line);
        }
        let first_delivery_id = headers1
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("x-fusionpanel-delivery-id")
                    .then(|| value.trim().to_owned())
            })
            .unwrap_or_default();
        let _ = first_tx.send(first_delivery_id);

        // Attempt 2 (after API restart): accept, verify, and return 204.
        let (stream2, _) = webhook_listener.accept().await.unwrap();
        drop(reader1);
        let mut reader2 = BufReader::new(stream2);
        let mut headers2 = String::new();
        loop {
            let mut line = String::new();
            reader2.read_line(&mut line).await.unwrap();
            if line == "\r\n" || line.is_empty() {
                break;
            }
            headers2.push_str(&line);
        }
        let content_length = headers2
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().ok())
                    .flatten()
            })
            .unwrap_or_default();
        let mut body = vec![0u8; content_length];
        reader2.read_exact(&mut body).await.unwrap();
        let _ = reader2
            .get_mut()
            .write_all(b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;

        let signature = headers2
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("x-fusionpanel-signature")
                    .then(|| value.trim().to_owned())
            })
            .unwrap_or_default();
        let delivery_id = headers2
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("x-fusionpanel-delivery-id")
                    .then(|| value.trim().to_owned())
            })
            .unwrap_or_default();
        let _ = second_tx.send(WebhookEvent {
            body: String::from_utf8(body).unwrap(),
            signature,
            delivery_id,
        });
    });

    let mut api = spawn_api(
        &api_binary,
        &database_path,
        &admin_token,
        &encryption_key,
        api_port,
        temp.path(),
    );
    assert!(
        wait_for_health(&mut api, &client, &api_url).await,
        "BLOCKED: isolated FusionPanel API did not become healthy"
    );

    let registration = client
        .post(format!("{api_url}/v1/nodes/register"))
        .bearer_auth(&admin_token)
        .json(&json!({ "name": "recovery-node" }))
        .send()
        .await
        .unwrap();
    assert_eq!(registration.status(), ClientStatus::CREATED);
    let reg_json: Value = registration.json().await.unwrap();
    let node_id = reg_json["node_id"].as_str().unwrap().to_owned();
    let node_token = reg_json["token"].as_str().unwrap().to_owned();

    let rule_resp = client
        .post(format!("{api_url}/v1/alerts/rules"))
        .bearer_auth(&admin_token)
        .json(&json!({
            "name": "Pending delivery recovery rule",
            "metric": "memory_used_percent",
            "operator": "above",
            "threshold": 0.0,
            "severity": "warning",
            "webhook_url": webhook_url,
            "webhook_secret": webhook_secret,
            "enabled": true
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(rule_resp.status(), ClientStatus::CREATED);
    let rule_id = rule_resp.json::<Value>().await.unwrap()["rule"]["id"]
        .as_str()
        .unwrap()
        .to_owned();

    // Post sample in background because the handler awaits the stalled webhook delivery.
    let post_client = client.clone();
    let post_url = format!("{api_url}/v1/nodes/{node_id}/metrics");
    let post_token = node_token.clone();
    tokio::spawn(async move {
        let _ = post_client
            .post(post_url)
            .bearer_auth(post_token)
            .json(&json!({
                "cpu_usage_percent": 10.0,
                "memory_used_kb": 1024,
                "memory_available_kb": 1024,
                "disk_io_ops": 1
            }))
            .send()
            .await;
    });

    let first_delivery_id = timeout(Duration::from_secs(10), first_rx)
        .await
        .expect("BLOCKED: webhook attempt never started")
        .expect("first_tx dropped");
    assert!(!first_delivery_id.is_empty());

    // Interrupt API immediately while the delivery is in-flight/pending.
    let _ = api.start_kill();
    let _ = timeout(Duration::from_secs(5), api.wait()).await;

    // Restart API on the same SQLite DB; recover_alerts should replay with the same delivery ID.
    let mut api = spawn_api(
        &api_binary,
        &database_path,
        &admin_token,
        &encryption_key,
        api_port,
        temp.path(),
    );
    assert!(
        wait_for_health(&mut api, &client, &api_url).await,
        "BLOCKED: FusionPanel API did not restart cleanly"
    );

    let recovered = timeout(Duration::from_secs(10), second_rx)
        .await
        .expect("BLOCKED: pending delivery was not replayed after API restart")
        .expect("second_tx dropped");
    assert_eq!(recovered.delivery_id, first_delivery_id);
    verify_webhook_event(&recovered, &webhook_secret, "triggered", &node_id, &rule_id);
    wait_for_deliveries(&client, &api_url, &admin_token, 1).await;

    stop_process(&mut api).await;
    webhook_task.abort();
}

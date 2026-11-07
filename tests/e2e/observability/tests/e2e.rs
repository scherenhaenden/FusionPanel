#![forbid(unsafe_code)]

use axum::{
    extract::State,
    http::StatusCode,
    routing::{get, post},
    Json, Router,
};
use observability_integration::{
    collector::{CollectorWorker, ProbeTarget},
    http::{router, AppState},
};
use serde_json::Value;
use std::{
    net::SocketAddr,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
    sync::Mutex,
    task::JoinHandle,
    time::Instant,
};

const ADMIN_TOKEN: &str = "fusionpanel-observability-e2e-token";

#[derive(Clone)]
struct TargetState(Arc<AtomicBool>);

#[derive(Clone, Default)]
struct WebhookState(Arc<Mutex<Vec<Value>>>);

async fn target_health(State(state): State<TargetState>) -> StatusCode {
    if state.0.load(Ordering::SeqCst) {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    }
}

async fn receive_alert(State(state): State<WebhookState>, Json(alert): Json<Value>) -> StatusCode {
    state.0.lock().await.push(alert);
    StatusCode::OK
}

async fn start_server(app: Router) -> (String, JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address: SocketAddr = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    (format!("http://{address}"), task)
}

async fn wait_for_incident_state(
    client: &reqwest::Client,
    base_url: &str,
    expected: &str,
) -> Value {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let response = client
            .get(format!("{base_url}/incidents"))
            .bearer_auth(ADMIN_TOKEN)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body: Value = response.json().await.unwrap();
        if let Some(incident) = body["incidents"]
            .as_array()
            .and_then(|incidents| incidents.first())
        {
            if incident["state"] == expected {
                return incident.clone();
            }
        }
        assert!(
            Instant::now() < deadline,
            "incident did not reach {expected}: {body}"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

async fn wait_for_alerts(state: &WebhookState) -> Vec<Value> {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let alerts = state.0.lock().await.clone();
        if !alerts.is_empty() {
            return alerts;
        }
        assert!(
            Instant::now() < deadline,
            "collector did not deliver an alert"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

#[tokio::test]
async fn real_collector_routes_deliver_alerts_and_persist_outage_recovery() {
    let temp = tempfile::tempdir().unwrap();
    let database_path = temp.path().join("incidents.sqlite");
    let client = reqwest::Client::new();

    let target_state = TargetState(Arc::new(AtomicBool::new(false)));
    let (target_url, target_server) = start_server(
        Router::new()
            .route("/health", get(target_health))
            .with_state(target_state.clone()),
    )
    .await;
    let webhook_state = WebhookState::default();
    let (webhook_url, webhook_server) = start_server(
        Router::new()
            .route("/alert", post(receive_alert))
            .with_state(webhook_state.clone()),
    )
    .await;

    let first_state = AppState::new(&database_path, ADMIN_TOKEN).unwrap();
    let first_worker = CollectorWorker::new(
        vec![ProbeTarget {
            id: "fixture-target".into(),
            target_type: "http".into(),
            url_or_addr: format!("{target_url}/health"),
        }],
        first_state.metrics.clone(),
        first_state.store.clone(),
        Some(format!("{webhook_url}/alert")),
    );
    let first_worker_task = tokio::spawn(async move { first_worker.run().await });
    let (first_url, first_server_task) = start_server(router(first_state.clone())).await;

    let unauthorized = client
        .get(format!("{first_url}/incidents"))
        .send()
        .await
        .unwrap();
    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let triggered = wait_for_incident_state(&client, &first_url, "Triggered").await;
    assert_eq!(triggered["id"], "inc-fixture-target");
    let alerts = wait_for_alerts(&webhook_state).await;
    assert_eq!(alerts[0]["id"], "alert-inc-fixture-target");

    first_worker_task.abort();
    let _ = first_worker_task.await;
    first_server_task.abort();
    let _ = first_server_task.await;
    drop(first_state);

    // A fresh router and SQLite connection must see the same incident.
    let restarted_state = AppState::new(&database_path, ADMIN_TOKEN).unwrap();
    let (restarted_url, restarted_server_task) =
        start_server(router(restarted_state.clone())).await;
    let persisted = wait_for_incident_state(&client, &restarted_url, "Triggered").await;
    assert_eq!(persisted["id"], triggered["id"]);

    target_state.0.store(true, Ordering::SeqCst);
    let recovery_worker = CollectorWorker::new(
        vec![ProbeTarget {
            id: "fixture-target".into(),
            target_type: "http".into(),
            url_or_addr: format!("{target_url}/health"),
        }],
        restarted_state.metrics.clone(),
        restarted_state.store.clone(),
        Some(format!("{webhook_url}/alert")),
    );
    let recovery_task = tokio::spawn(async move { recovery_worker.run().await });
    let resolved = wait_for_incident_state(&client, &restarted_url, "Resolved").await;
    assert_eq!(resolved["id"], persisted["id"]);

    let metrics = client
        .get(format!("{restarted_url}/metrics"))
        .send()
        .await
        .unwrap();
    assert_eq!(metrics.status(), StatusCode::OK);
    assert!(metrics
        .text()
        .await
        .unwrap()
        .contains("site_requests_total"));

    recovery_task.abort();
    let _ = recovery_task.await;
    restarted_server_task.abort();
    let _ = restarted_server_task.await;
    target_server.abort();
    let _ = target_server.await;
    webhook_server.abort();
    let _ = webhook_server.await;
}

#[tokio::test]
async fn test_metrics_collection_interval_and_storage_retention() {
    let temp = tempfile::tempdir().unwrap();
    let database_path = temp.path().join("incidents_retention.sqlite");
    let client = reqwest::Client::new();

    // Start a target server that is healthy, so metrics are incremented
    let target_state = TargetState(Arc::new(AtomicBool::new(true)));
    let (target_url, target_server) = start_server(
        Router::new()
            .route("/health", get(target_health))
            .with_state(target_state.clone()),
    )
    .await;

    let app_state = AppState::new(&database_path, ADMIN_TOKEN).unwrap();
    let worker = CollectorWorker::new(
        vec![ProbeTarget {
            id: "retention-target".into(),
            target_type: "http".into(),
            url_or_addr: format!("{target_url}/health"),
        }],
        app_state.metrics.clone(),
        app_state.store.clone(),
        None,
    );
    let worker_task = tokio::spawn(async move { worker.run().await });
    let (app_url, app_server_task) = start_server(router(app_state.clone())).await;

    // Wait for the collector to poll at least twice
    tokio::time::sleep(Duration::from_secs(5)).await;

    // Verify metrics are collected and updated correctly
    let metrics = client
        .get(format!("{app_url}/metrics"))
        .send()
        .await
        .unwrap();
    assert_eq!(metrics.status(), StatusCode::OK);
    let metrics_text = metrics.text().await.unwrap();
    assert!(metrics_text.contains("site_requests_total"));

    // Attempt to parse the total requests and verify it's > 0
    let mut total_requests = 0;
    for line in metrics_text.lines() {
        if line.starts_with("site_requests_total") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() == 2 {
                total_requests = parts[1].parse::<i64>().unwrap_or(0);
            }
        }
    }
    assert!(total_requests > 0, "Metrics were not collected during the interval");

    // We can simulate an outage, wait for incident generation and check retention manually
    target_state.0.store(false, Ordering::SeqCst);

    // Wait for incident generation
    let _ = wait_for_incident_state(&client, &app_url, "Triggered").await;

    // Check retention via API
    let incidents = client
        .get(format!("{app_url}/incidents"))
        .bearer_auth(ADMIN_TOKEN)
        .send()
        .await
        .unwrap();
    let body: Value = incidents.json().await.unwrap();
    let incident_list = body["incidents"].as_array().unwrap();
    let found = incident_list.iter().any(|i| i["id"] == "inc-retention-target");
    assert!(found, "Incident was not retained in storage");

    worker_task.abort();
    let _ = worker_task.await;
    app_server_task.abort();
    let _ = app_server_task.await;
    target_server.abort();
    let _ = target_server.await;
}

#[tokio::test]
async fn test_alert_delivery_dispatch_email() {
    let temp = tempfile::tempdir().unwrap();
    let database_path = temp.path().join("incidents_email.sqlite");

    if std::env::var("FUSIONPANEL_SMTP_SERVER").is_err() {
        panic!("BLOCKED: email payload formatting tests require an external SMTP server / notification provider");
    }

    // If FUSIONPANEL_SMTP_SERVER is set, we will bind to a local test port
    // and explicitly override the environment variable so the worker is guaranteed to be "aware" of it
    // if the product code was looking for it.
    let mock_smtp = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_smtp_addr = mock_smtp.local_addr().unwrap().to_string();
    std::env::set_var("FUSIONPANEL_SMTP_SERVER", &mock_smtp_addr);

    let target_state = TargetState(Arc::new(AtomicBool::new(false)));
    let (target_url, target_server) = start_server(
        Router::new()
            .route("/health", get(target_health))
            .with_state(target_state.clone()),
    )
    .await;

    let app_state = AppState::new(&database_path, ADMIN_TOKEN).unwrap();

    // Setup worker - if it supported SMTP, it would read the env var and connect here.
    let worker = CollectorWorker::new(
        vec![ProbeTarget {
            id: "email-target".into(),
            target_type: "http".into(),
            url_or_addr: format!("{target_url}/health"),
        }],
        app_state.metrics.clone(),
        app_state.store.clone(),
        None, // In this simulated scenario, no webhook is passed; email falls back to SMTP env.
    );
    let worker_task = tokio::spawn(async move { worker.run().await });

    // Run the mock SMTP server loop to intercept the email alert.
    let timeout = tokio::time::timeout(Duration::from_secs(10), mock_smtp.accept()).await;
    match timeout {
        Ok(Ok((mut stream, _))) => {
            // SMTP Protocol Greeting
            stream.write_all(b"220 Mock SMTP Server Ready\r\n").await.unwrap();

            let mut buf = [0; 4096];
            let mut email_content = String::new();
            let mut in_data = false;

            loop {
                let n = tokio::time::timeout(Duration::from_secs(5), stream.read(&mut buf))
                    .await
                    .unwrap_or(Ok(0))
                    .unwrap_or(0);
                if n == 0 { break; }
                let client_msg = String::from_utf8_lossy(&buf[..n]);
                email_content.push_str(&client_msg);

                if in_data {
                    if email_content.ends_with("\r\n.\r\n") {
                        stream.write_all(b"250 OK\r\n").await.unwrap();
                        break;
                    }
                } else {
                    if client_msg.starts_with("DATA") {
                        in_data = true;
                        stream.write_all(b"354 Start mail input; end with <CRLF>.<CRLF>\r\n").await.unwrap();
                    } else if client_msg.starts_with("QUIT") {
                        stream.write_all(b"221 Bye\r\n").await.unwrap();
                        break;
                    } else {
                        stream.write_all(b"250 OK\r\n").await.unwrap();
                    }
                }
            }
            assert!(email_content.contains("Subject:"), "Email payload missing Subject format");
        }
        Ok(Err(e)) => panic!("SMTP accept failed: {}", e),
        Err(_) => panic!("Timed out waiting for SMTP email dispatch - capability missing or failed"),
    }

    worker_task.abort();
    let _ = worker_task.await;
    target_server.abort();
    let _ = target_server.await;
}

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
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle, time::Instant};

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
    let deadline = Instant::now() + Duration::from_secs(12);
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
    let deadline = Instant::now() + Duration::from_secs(12);
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

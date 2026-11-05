#![forbid(unsafe_code)]

use observability_integration::{
    collector::{CollectorWorker, ProbeTarget},
    http::{router, AppState},
};
use std::env;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::new(
            env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
        ))
        .with(tracing_subscriber::fmt::layer())
        .init();

    let store_path =
        env::var("FUSIONPANEL_INCIDENTS_DB").unwrap_or_else(|_| "incidents.db".to_string());
    let admin_token = env::var("FUSIONPANEL_ADMIN_TOKEN").unwrap_or_default();
    let state = AppState::new(std::path::Path::new(&store_path), admin_token)
        .expect("Failed to initialize observability state");

    let target_url = env::var("TEST_TARGET_URL").unwrap_or_default();
    let webhook_url = env::var("TEST_WEBHOOK_URL").ok();
    let targets = if target_url.is_empty() {
        Vec::new()
    } else {
        vec![ProbeTarget {
            id: "target-1".to_string(),
            target_type: if target_url.starts_with("http") {
                "http".to_string()
            } else {
                "tcp".to_string()
            },
            url_or_addr: target_url,
        }]
    };

    let worker = CollectorWorker::new(
        targets,
        state.metrics.clone(),
        state.store.clone(),
        webhook_url,
    );
    tokio::spawn(async move {
        worker.run().await;
    });

    let listen_addr =
        env::var("FUSIONPANEL_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&listen_addr).await.unwrap();
    tracing::info!("listening on {}", listener.local_addr().unwrap());
    axum::serve(listener, router(state)).await.unwrap();
}

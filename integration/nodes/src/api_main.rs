use axum::serve;
use integration_nodes::router::{create_router, AppState};

use std::sync::Arc;
use tokio::net::TcpListener;

#[tokio::main]
async fn main() {
    let token =
        std::env::var("FUSIONPANEL_ADMIN_TOKEN").expect("FUSIONPANEL_ADMIN_TOKEN must be set");
    if token.is_empty() {
        panic!("FUSIONPANEL_ADMIN_TOKEN must not be empty");
    }
    let listen =
        std::env::var("FUSIONPANEL_LISTEN").unwrap_or_else(|_| "127.0.0.1:8080".to_string());
    let state = Arc::new(AppState { token });

    let app = create_router(state);

    let listener = TcpListener::bind(&listen).await.unwrap();
    serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .unwrap();
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("failed to install Ctrl+C handler");
}

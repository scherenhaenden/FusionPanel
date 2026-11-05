use administration::{audit::AuditLog, quotas::QuotaEngine};
use axum::Router;
use integration_administration::routes::{AppState, DurableInvitations};
use std::{net::SocketAddr, path::Path, sync::Arc};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};

pub const ADMIN_TOKEN: &str = "fusionpanel-e2e-admin-token";

pub struct RunningServer {
    pub base_url: String,
    task: JoinHandle<()>,
}

impl RunningServer {
    pub async fn start(app: Router) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address: SocketAddr = listener.local_addr().unwrap();
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self {
            base_url: format!("http://{address}"),
            task,
        }
    }

    pub async fn stop(self) {
        self.task.abort();
        let _ = self.task.await;
    }
}

pub fn app_state(audit_path: &Path, invitations_path: &Path) -> AppState {
    AppState {
        quotas: Arc::new(QuotaEngine::new()),
        audit: Arc::new(Mutex::new(AuditLog::with_path(audit_path).unwrap())),
        invitations: Arc::new(Mutex::new(DurableInvitations::new(
            invitations_path.to_path_buf(),
        ))),
        admin_token: Arc::new(ADMIN_TOKEN.to_string()),
    }
}

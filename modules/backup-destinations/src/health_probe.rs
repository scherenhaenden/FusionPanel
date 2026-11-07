#![forbid(unsafe_code)]

use crate::traits::StorageBackend;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unreachable,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DestinationHealth {
    pub status: HealthStatus,
    pub last_latency: Option<Duration>,
    pub last_error: Option<String>,
}

impl Default for DestinationHealth {
    fn default() -> Self {
        Self {
            status: HealthStatus::Unknown,
            last_latency: None,
            last_error: None,
        }
    }
}

pub struct HealthProbe<B: StorageBackend> {
    pub backend: B,
    pub health: Arc<Mutex<DestinationHealth>>,
}

impl<B: StorageBackend> HealthProbe<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            health: Arc::new(Mutex::new(DestinationHealth::default())),
        }
    }

    pub async fn run_probe(&self, probe_path: &str) {
        let start = Instant::now();
        let payload = b"health_probe_data".to_vec();

        let mut next_health = DestinationHealth {
            status: HealthStatus::Unreachable,
            last_latency: None,
            last_error: None,
        };

        match self.backend.upload(probe_path, payload.clone()).await {
            Ok(_) => match self.backend.download(probe_path).await {
                Ok(data) => {
                    if data == payload {
                        match self.backend.delete(probe_path).await {
                            Ok(_) => {
                                next_health.status = HealthStatus::Healthy;
                            }
                            Err(e) => {
                                next_health.status = HealthStatus::Degraded;
                                next_health.last_error = Some(format!("Delete failed: {}", e));
                            }
                        }
                    } else {
                        let _ = self.backend.delete(probe_path).await;
                        next_health.status = HealthStatus::Degraded;
                        next_health.last_error = Some("Data mismatch on download".to_string());
                    }
                }
                Err(e) => {
                    let _ = self.backend.delete(probe_path).await;
                    next_health.status = HealthStatus::Degraded;
                    next_health.last_error = Some(format!("Download failed: {}", e));
                }
            },
            Err(e) => {
                next_health.status = HealthStatus::Unreachable;
                next_health.last_error = Some(format!("Upload failed: {}", e));
            }
        }

        next_health.last_latency = Some(start.elapsed());

        let mut lock = self.health.lock().unwrap();
        *lock = next_health;
    }

    pub fn get_health(&self) -> DestinationHealth {
        self.health.lock().unwrap().clone()
    }
}

use observability::{
    alerts::{Alert, AlertDelivery, AlertSeverity, WebhookDelivery},
    incidents::{Incident, IncidentState},
    metrics::MetricsRegistry,
    probes::{check_http, check_tcp},
    store::IncidentStore,
};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::sync::Mutex;
use tokio::time::sleep;

pub struct ProbeTarget {
    pub id: String,
    pub target_type: String, // "http" or "tcp"
    pub url_or_addr: String,
}

pub struct CollectorWorker {
    targets: Vec<ProbeTarget>,
    metrics: Arc<MetricsRegistry>,
    store: Arc<Mutex<IncidentStore>>,
    webhook_url: Option<String>,
}

impl CollectorWorker {
    pub fn new(
        targets: Vec<ProbeTarget>,
        metrics: Arc<MetricsRegistry>,
        store: Arc<Mutex<IncidentStore>>,
        webhook_url: Option<String>,
    ) -> Self {
        Self {
            targets,
            metrics,
            store,
            webhook_url,
        }
    }

    pub async fn run(&self) {
        loop {
            for target in &self.targets {
                // Use spawn_blocking for blocking check calls to prevent blocking the async thread
                let url_or_addr = target.url_or_addr.clone();
                let target_type = target.target_type.clone();

                let success = tokio::task::spawn_blocking(move || match target_type.as_str() {
                    "http" => check_http(&url_or_addr, 5).success,
                    "tcp" => check_tcp(&url_or_addr, 5).success,
                    _ => false,
                })
                .await
                .unwrap_or(false);

                let now = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or(Duration::from_secs(0))
                    .as_secs();

                let store_guard = self.store.lock().await;
                let incident_id = format!("inc-{}", target.id);

                let existing_incident = store_guard.get_incident(&incident_id).unwrap_or(None);

                if success {
                    // Update metrics
                    self.metrics.site_requests.inc();

                    // Resolve incident if it exists and is not resolved
                    if let Some(mut incident) = existing_incident {
                        if incident.state != IncidentState::Resolved {
                            let _ = incident.resolve(now);
                            let _ = store_guard.save_incident(&incident);
                        }
                    }
                } else {
                    // Target failed. Create or trigger incident.
                    let mut incident = existing_incident.unwrap_or_else(|| {
                        Incident::new(
                            incident_id.clone(),
                            format!("Outage on {}", target.id),
                            now,
                            None,
                        )
                    });

                    if incident.state == IncidentState::Resolved {
                        // Reopen the incident
                        incident.state = IncidentState::Triggered;
                        incident.updated_at = now;
                    }

                    if incident.state == IncidentState::Triggered && incident.updated_at == now {
                        // Newly triggered or reopened at this exact moment, send alert once
                        if let Some(webhook_url) = &self.webhook_url {
                            let alert = Alert {
                                id: format!("alert-{}", incident_id),
                                title: incident.title.clone(),
                                description: format!(
                                    "Target {} is unreachable",
                                    target.url_or_addr
                                ),
                                severity: AlertSeverity::Critical,
                                source: "collector".to_string(),
                                timestamp: now,
                            };

                            let delivery = WebhookDelivery {
                                url: webhook_url.clone(),
                            };
                            drop(tokio::spawn(async move {
                                let _ = tokio::task::spawn_blocking(move || {
                                    let mut attempts = 0;
                                    let max_attempts = 3;
                                    while attempts < max_attempts {
                                        if delivery.deliver(&alert).is_ok() {
                                            break;
                                        }
                                        attempts += 1;
                                        std::thread::sleep(Duration::from_secs(1 << attempts));
                                    }
                                })
                                .await;
                            }));
                        }
                    }

                    let _ = store_guard.save_incident(&incident);
                }
            }

            // Explicit drop to avoid holding lock during sleep
            // which can happen depending on loop structure though we aren't here

            sleep(Duration::from_secs(2)).await;
        }
    }
}

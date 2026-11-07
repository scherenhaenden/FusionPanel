#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::time::Duration;
use chrono::{DateTime, Utc};
use crate::incident::{Incident, IncidentState};

pub trait NotificationChannel {
    fn send(&self, incident: &Incident) -> Result<(), String>;
}

pub struct WebhookNotificationChannel {
    pub url: String,
}

impl WebhookNotificationChannel {
    pub fn new(url: String) -> Self {
        Self { url }
    }
}

impl NotificationChannel for WebhookNotificationChannel {
    fn send(&self, _incident: &Incident) -> Result<(), String> {
        // In a real implementation this would make an HTTP request.
        // For now, we simulate a successful send.
        Ok(())
    }
}

pub struct AlertNotifier<C: NotificationChannel> {
    channel: C,
    cooldown: Duration,
    last_notified: HashMap<String, DateTime<Utc>>,
}

impl<C: NotificationChannel> AlertNotifier<C> {
    pub fn new(channel: C, cooldown: Duration) -> Self {
        Self {
            channel,
            cooldown,
            last_notified: HashMap::new(),
        }
    }

    pub fn dispatch(&mut self, incident: &Incident) -> Result<bool, String> {
        // Open incidents are subject to cooldown logic.
        // Acknowledged and Resolved incidents bypass cooldown.
        // Resolved incidents remove the cooldown tracking entirely.

        let now = Utc::now();
        let rule_id = incident.rule_id.clone();

        match incident.state {
            IncidentState::Open => {
                if let Some(last_time) = self.last_notified.get(&rule_id) {
                    let elapsed = (now - *last_time).to_std().unwrap_or(Duration::ZERO);
                    if elapsed < self.cooldown {
                        // Cooldown active, do not dispatch
                        return Ok(false);
                    }
                }

                // Proceed to send
                self.channel.send(incident)?;
                self.last_notified.insert(rule_id, now);
                Ok(true)
            }
            IncidentState::Acknowledged => {
                // Acknowledge always dispatches and updates last_notified if we want to reset cooldown?
                // The requirements say: "State transitions (Acknowledged, Resolved) bypass cooldown"
                self.channel.send(incident)?;
                // Update cooldown timer on Ack? Requirements say bypass, meaning we always send.
                // It makes sense to reset the cooldown timer on any dispatch.
                self.last_notified.insert(rule_id, now);
                Ok(true)
            }
            IncidentState::Resolved => {
                self.channel.send(incident)?;
                // Resolving resets/removes cooldown
                self.last_notified.remove(&rule_id);
                Ok(true)
            }
        }
    }
}

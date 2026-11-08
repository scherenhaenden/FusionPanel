#![forbid(unsafe_code)]

use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

impl Severity {
    pub fn escalate(&self) -> Self {
        match self {
            Severity::Low => Severity::Medium,
            Severity::Medium => Severity::High,
            Severity::High => Severity::Critical,
            Severity::Critical => Severity::Critical,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentState {
    Open,
    Acknowledged,
    Resolved,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AlertFingerprint {
    pub source: String,
    pub resource: String,
    pub severity: Severity,
}

#[derive(Debug, Clone)]
pub struct Alert {
    pub fingerprint: AlertFingerprint,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventType {
    Created,
    StateChanged { from: IncidentState, to: IncidentState },
    SeverityEscalated { from: Severity, to: Severity },
    NoteAdded(String),
    AlertRepeated(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimelineEvent {
    pub timestamp: DateTime<Utc>,
    pub event_type: EventType,
}

#[derive(Debug, Clone)]
pub struct Incident {
    pub id: Uuid,
    pub fingerprint: AlertFingerprint,
    pub current_severity: Severity,
    pub state: IncidentState,
    pub timeline: Vec<TimelineEvent>,
    pub created_at: DateTime<Utc>,
    pub last_escalated_at: DateTime<Utc>,
    pub last_alert_time: DateTime<Utc>,
    pub acknowledged_at: Option<DateTime<Utc>>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub closed_at: Option<DateTime<Utc>>,
    pub duplicate_count: u32,
    pub latest_alert_message: String,
}

#[derive(Debug, Error)]
pub enum IncidentError {
    #[error("Incident not found")]
    NotFound,
    #[error("Invalid state transition")]
    InvalidTransition,
}

pub struct IncidentManager {
    incidents: HashMap<Uuid, Incident>,
    active_incidents: HashMap<AlertFingerprint, Uuid>,
    dedup_window: Duration,
    escalation_timeout: Duration,
}

impl IncidentManager {
    pub fn new(dedup_window: Duration, escalation_timeout: Duration) -> Self {
        Self {
            incidents: HashMap::new(),
            active_incidents: HashMap::new(),
            dedup_window,
            escalation_timeout,
        }
    }

    pub fn receive_alert(&mut self, alert: Alert, now: DateTime<Utc>) -> Uuid {
        let fingerprint = alert.fingerprint.clone();

        if let Some(&id) = self.active_incidents.get(&fingerprint) {
            let incident = self.incidents.get_mut(&id).expect("Incident should exist if in active_incidents");

            let time_since_last = now.signed_duration_since(incident.last_alert_time);
            if time_since_last <= self.dedup_window {
                incident.duplicate_count = incident.duplicate_count.saturating_add(1);
            } else {
                incident.timeline.push(TimelineEvent {
                    timestamp: now,
                    event_type: EventType::AlertRepeated(alert.message.clone()),
                });
            }

            incident.last_alert_time = now;
            incident.latest_alert_message = alert.message;
            return id;
        }

        let id = Uuid::new_v4();
        let incident = Incident {
            id,
            fingerprint: fingerprint.clone(),
            current_severity: fingerprint.severity,
            state: IncidentState::Open,
            timeline: vec![TimelineEvent {
                timestamp: now,
                event_type: EventType::Created,
            }],
            created_at: now,
            last_escalated_at: now,
            last_alert_time: now,
            acknowledged_at: None,
            resolved_at: None,
            closed_at: None,
            duplicate_count: 0,
            latest_alert_message: alert.message,
        };

        self.incidents.insert(id, incident);
        self.active_incidents.insert(fingerprint, id);
        id
    }

    pub fn acknowledge(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), IncidentError> {
        let incident = self.incidents.get_mut(&id).ok_or(IncidentError::NotFound)?;
        if incident.state != IncidentState::Open {
            return Err(IncidentError::InvalidTransition);
        }
        incident.state = IncidentState::Acknowledged;
        incident.acknowledged_at = Some(now);
        incident.timeline.push(TimelineEvent {
            timestamp: now,
            event_type: EventType::StateChanged { from: IncidentState::Open, to: IncidentState::Acknowledged },
        });
        Ok(())
    }

    pub fn resolve(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), IncidentError> {
        let incident = self.incidents.get_mut(&id).ok_or(IncidentError::NotFound)?;
        if incident.state != IncidentState::Acknowledged {
            return Err(IncidentError::InvalidTransition);
        }
        incident.state = IncidentState::Resolved;
        incident.resolved_at = Some(now);
        incident.timeline.push(TimelineEvent {
            timestamp: now,
            event_type: EventType::StateChanged { from: IncidentState::Acknowledged, to: IncidentState::Resolved },
        });
        Ok(())
    }

    pub fn close(&mut self, id: Uuid, now: DateTime<Utc>) -> Result<(), IncidentError> {
        let incident = self.incidents.get_mut(&id).ok_or(IncidentError::NotFound)?;
        if incident.state != IncidentState::Resolved {
            return Err(IncidentError::InvalidTransition);
        }
        incident.state = IncidentState::Closed;
        incident.closed_at = Some(now);
        incident.timeline.push(TimelineEvent {
            timestamp: now,
            event_type: EventType::StateChanged { from: IncidentState::Resolved, to: IncidentState::Closed },
        });

        if self.active_incidents.get(&incident.fingerprint) == Some(&id) {
            self.active_incidents.remove(&incident.fingerprint);
        }

        Ok(())
    }

    pub fn add_note(&mut self, id: Uuid, note: String, now: DateTime<Utc>) -> Result<(), IncidentError> {
        let incident = self.incidents.get_mut(&id).ok_or(IncidentError::NotFound)?;
        incident.timeline.push(TimelineEvent {
            timestamp: now,
            event_type: EventType::NoteAdded(note),
        });
        Ok(())
    }

    pub fn process_escalations(&mut self, now: DateTime<Utc>) {
        let mut to_update = Vec::new();
        for (id, incident) in &self.incidents {
            if incident.state == IncidentState::Open {
                if now.signed_duration_since(incident.last_escalated_at) > self.escalation_timeout {
                    if incident.current_severity < Severity::Critical {
                        to_update.push(*id);
                    }
                }
            }
        }

        for id in to_update {
            if let Some(mut incident) = self.incidents.remove(&id) {
                let old_sev = incident.current_severity;
                let new_sev = old_sev.escalate();

                if self.active_incidents.get(&incident.fingerprint) == Some(&id) {
                    self.active_incidents.remove(&incident.fingerprint);
                }

                incident.current_severity = new_sev;
                incident.fingerprint.severity = new_sev;
                incident.last_escalated_at = now;
                incident.timeline.push(TimelineEvent {
                    timestamp: now,
                    event_type: EventType::SeverityEscalated { from: old_sev, to: new_sev },
                });

                self.active_incidents.insert(incident.fingerprint.clone(), id);
                self.incidents.insert(id, incident);
            }
        }
    }

    pub fn mtta(&self) -> Option<Duration> {
        let mut total_ms = 0;
        let mut count = 0;

        for incident in self.incidents.values() {
            if let Some(ack) = incident.acknowledged_at {
                total_ms += ack.signed_duration_since(incident.created_at).num_milliseconds();
                count += 1;
            }
        }

        if count > 0 {
            Some(Duration::milliseconds(total_ms / count))
        } else {
            None
        }
    }

    pub fn mttr(&self) -> Option<Duration> {
        let mut total_ms = 0;
        let mut count = 0;

        for incident in self.incidents.values() {
            if let Some(res) = incident.resolved_at {
                total_ms += res.signed_duration_since(incident.created_at).num_milliseconds();
                count += 1;
            }
        }

        if count > 0 {
            Some(Duration::milliseconds(total_ms / count))
        } else {
            None
        }
    }

    pub fn get_incident(&self, id: Uuid) -> Option<&Incident> {
        self.incidents.get(&id)
    }
}

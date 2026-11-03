use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum IncidentState {
    Triggered,
    Acknowledged,
    Resolved,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Incident {
    pub id: String,
    pub tenant_id: Option<String>,
    pub title: String,
    pub state: IncidentState,
    pub created_at: u64,
    pub updated_at: u64,
}

impl Incident {
    pub fn new(id: String, title: String, timestamp: u64, tenant_id: Option<String>) -> Self {
        Self {
            id,
            tenant_id,
            title,
            state: IncidentState::Triggered,
            created_at: timestamp,
            updated_at: timestamp,
        }
    }

    pub fn acknowledge(&mut self, timestamp: u64) -> Result<(), String> {
        if self.state == IncidentState::Triggered {
            self.state = IncidentState::Acknowledged;
            self.updated_at = timestamp;
            Ok(())
        } else {
            Err("Can only acknowledge triggered incidents".into())
        }
    }

    pub fn resolve(&mut self, timestamp: u64) -> Result<(), String> {
        if self.state != IncidentState::Resolved {
            self.state = IncidentState::Resolved;
            self.updated_at = timestamp;
            Ok(())
        } else {
            Err("Incident is already resolved".into())
        }
    }
}

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
pub enum IncidentState {
    Open,
    Acknowledged,
    Resolved,
}

#[derive(Debug, Clone)]
pub struct Incident {
    pub id: Uuid,
    pub rule_id: String,
    pub state: IncidentState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl Incident {
    pub fn new(rule_id: String) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4(),
            rule_id,
            state: IncidentState::Open,
            created_at: now,
            updated_at: now,
        }
    }

    pub fn acknowledge(&mut self) -> Result<(), String> {
        if self.state == IncidentState::Open {
            self.state = IncidentState::Acknowledged;
            self.updated_at = Utc::now();
            Ok(())
        } else {
            Err(format!("Cannot acknowledge incident from state {:?}", self.state))
        }
    }

    pub fn resolve(&mut self) -> Result<(), String> {
        if self.state == IncidentState::Open || self.state == IncidentState::Acknowledged {
            self.state = IncidentState::Resolved;
            self.updated_at = Utc::now();
            Ok(())
        } else {
            Err(format!("Cannot resolve incident from state {:?}", self.state))
        }
    }
}

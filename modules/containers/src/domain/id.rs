use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

use super::error::DomainError;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ContainerId(String);

impl ContainerId {
    pub fn new(id: String) -> Result<Self, DomainError> {
        if id.is_empty() {
            return Err(DomainError::InvalidContainerId);
        }
        // Very basic validation - could be UUID or a 64 char hex string depending on the engine
        // Let's assume alphanumeric string between 1 and 128 chars for simplicity
        if id.len() > 128
            || !id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        {
            return Err(DomainError::InvalidContainerId);
        }
        Ok(Self(id))
    }

    pub fn generate() -> Self {
        Self(Uuid::new_v4().to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ContainerId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

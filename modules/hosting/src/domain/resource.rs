use serde::{Deserialize, Serialize};
use crate::domain::error::{DomainError, Result};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceState {
    Pending,
    Active,
    Suspended,
    Failed,
    Deleting,
    Deleted,
}

impl ResourceState {
    pub fn transition_to(&self, new_state: ResourceState) -> Result<ResourceState> {
        match (self, new_state) {
            (ResourceState::Pending, ResourceState::Active) |
            (ResourceState::Pending, ResourceState::Failed) |
            (ResourceState::Active, ResourceState::Suspended) |
            (ResourceState::Active, ResourceState::Deleting) |
            (ResourceState::Suspended, ResourceState::Active) |
            (ResourceState::Suspended, ResourceState::Deleting) |
            (ResourceState::Failed, ResourceState::Deleting) |
            (ResourceState::Failed, ResourceState::Pending) |
            (ResourceState::Deleting, ResourceState::Deleted) |
            (ResourceState::Deleting, ResourceState::Failed) => Ok(new_state),
            (current, target) if current == &target => Ok(new_state),
            (current, target) => Err(DomainError::Conflict(format!(
                "Invalid state transition from {:?} to {:?}",
                current, target
            ))),
        }
    }
}

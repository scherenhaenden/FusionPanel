use serde::{Deserialize, Serialize};

use super::error::DomainError;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ContainerStatus {
    Created,
    Running,
    Stopped,
    Exited,
    Removed,
}

impl ContainerStatus {
    pub fn transition_to(
        &self,
        next_state: ContainerStatus,
    ) -> Result<ContainerStatus, DomainError> {
        match (self, next_state) {
            // Created can transition to Running, Stopped (if setup fails/is cancelled), or Removed
            (ContainerStatus::Created, ContainerStatus::Running) => Ok(next_state),
            (ContainerStatus::Created, ContainerStatus::Stopped) => Ok(next_state),
            (ContainerStatus::Created, ContainerStatus::Removed) => Ok(next_state),

            // Running can transition to Stopped or Exited
            (ContainerStatus::Running, ContainerStatus::Stopped) => Ok(next_state),
            (ContainerStatus::Running, ContainerStatus::Exited) => Ok(next_state),

            // Stopped can transition to Running or Removed
            (ContainerStatus::Stopped, ContainerStatus::Running) => Ok(next_state),
            (ContainerStatus::Stopped, ContainerStatus::Removed) => Ok(next_state),

            // Exited can transition to Running (restarted) or Removed
            (ContainerStatus::Exited, ContainerStatus::Running) => Ok(next_state),
            (ContainerStatus::Exited, ContainerStatus::Removed) => Ok(next_state),

            // Same state transition is a no-op / success
            (current, next) if current == &next => Ok(next_state),

            // Any other transition is invalid
            (from, to) => Err(DomainError::InvalidStatusTransition { from: *from, to }),
        }
    }
}

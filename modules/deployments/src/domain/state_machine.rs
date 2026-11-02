use super::models::{Deployment, DeploymentStatus};
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TransitionError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition {
        from: DeploymentStatus,
        to: DeploymentStatus,
    },
}

impl Deployment {
    pub fn transition_to(&mut self, new_status: DeploymentStatus) -> Result<(), TransitionError> {
        let valid = match (self.status, new_status) {
            (DeploymentStatus::Pending, DeploymentStatus::Building) => true,
            (DeploymentStatus::Pending, DeploymentStatus::Failed) => true, // Fast fail
            (DeploymentStatus::Building, DeploymentStatus::Deploying) => true,
            (DeploymentStatus::Building, DeploymentStatus::Failed) => true,
            (DeploymentStatus::Deploying, DeploymentStatus::Success) => true,
            (DeploymentStatus::Deploying, DeploymentStatus::Failed) => true,
            (DeploymentStatus::Success, DeploymentStatus::RolledBack) => true,
            (DeploymentStatus::Failed, DeploymentStatus::RolledBack) => false, // Cannot rollback a failed deployment
            _ => false,
        };

        if valid {
            self.status = new_status;
            Ok(())
        } else {
            Err(TransitionError::InvalidTransition {
                from: self.status,
                to: new_status,
            })
        }
    }
}

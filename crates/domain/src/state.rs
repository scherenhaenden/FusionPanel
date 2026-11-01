use crate::error::DomainError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceState {
    Pending,
    Active,
    Suspended,
    Failed,
    Deleting,
    Deleted,
}

impl ResourceState {
    pub fn transition_to(&self, to: ResourceState) -> Result<ResourceState, DomainError> {
        let valid = match (self, to) {
            (ResourceState::Pending, ResourceState::Active) => true,
            (ResourceState::Pending, ResourceState::Failed) => true,
            (ResourceState::Pending, ResourceState::Deleting) => true,

            (ResourceState::Active, ResourceState::Suspended) => true,
            (ResourceState::Active, ResourceState::Deleting) => true,

            (ResourceState::Suspended, ResourceState::Active) => true,
            (ResourceState::Suspended, ResourceState::Deleting) => true,

            (ResourceState::Failed, ResourceState::Deleting) => true,

            (ResourceState::Deleting, ResourceState::Deleted) => true,
            (ResourceState::Deleting, ResourceState::Failed) => true,

            _ => false,
        };

        if valid {
            Ok(to)
        } else {
            Err(DomainError::InvalidStateTransition { from: *self, to })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_transitions() {
        assert_eq!(
            ResourceState::Pending
                .transition_to(ResourceState::Active)
                .unwrap(),
            ResourceState::Active
        );
        assert_eq!(
            ResourceState::Pending
                .transition_to(ResourceState::Failed)
                .unwrap(),
            ResourceState::Failed
        );
        assert_eq!(
            ResourceState::Pending
                .transition_to(ResourceState::Deleting)
                .unwrap(),
            ResourceState::Deleting
        );

        assert_eq!(
            ResourceState::Active
                .transition_to(ResourceState::Suspended)
                .unwrap(),
            ResourceState::Suspended
        );
        assert_eq!(
            ResourceState::Active
                .transition_to(ResourceState::Deleting)
                .unwrap(),
            ResourceState::Deleting
        );

        assert_eq!(
            ResourceState::Suspended
                .transition_to(ResourceState::Active)
                .unwrap(),
            ResourceState::Active
        );
        assert_eq!(
            ResourceState::Suspended
                .transition_to(ResourceState::Deleting)
                .unwrap(),
            ResourceState::Deleting
        );

        assert_eq!(
            ResourceState::Failed
                .transition_to(ResourceState::Deleting)
                .unwrap(),
            ResourceState::Deleting
        );

        assert_eq!(
            ResourceState::Deleting
                .transition_to(ResourceState::Deleted)
                .unwrap(),
            ResourceState::Deleted
        );
        assert_eq!(
            ResourceState::Deleting
                .transition_to(ResourceState::Failed)
                .unwrap(),
            ResourceState::Failed
        );
    }

    #[test]
    fn test_invalid_transitions() {
        assert!(ResourceState::Pending
            .transition_to(ResourceState::Suspended)
            .is_err());
        assert!(ResourceState::Pending
            .transition_to(ResourceState::Deleted)
            .is_err());

        assert!(ResourceState::Active
            .transition_to(ResourceState::Failed)
            .is_err());
        assert!(ResourceState::Active
            .transition_to(ResourceState::Deleted)
            .is_err());

        assert!(ResourceState::Deleted
            .transition_to(ResourceState::Active)
            .is_err());
    }

    #[test]
    fn test_state_serde() {
        let state = ResourceState::Pending;
        assert_eq!(serde_json::to_string(&state).unwrap(), "\"pending\"");

        let state = ResourceState::Active;
        assert_eq!(serde_json::to_string(&state).unwrap(), "\"active\"");
    }
}

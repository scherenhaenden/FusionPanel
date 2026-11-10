#![forbid(unsafe_code)]

use std::time::Duration;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum RolloverError {
    #[error("Invalid state transition from {from:?} to {to:?}")]
    InvalidTransition { from: KeyPhase, to: KeyPhase },
    #[error("Method {0:?} is not supported for key type {1:?}")]
    UnsupportedMethod(RolloverMethod, KeyType),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    KSK,
    ZSK,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RolloverMethod {
    PrePublish,
    DoubleSignature,
    DoubleDS,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyPhase {
    Generated,
    Published,
    Active,
    Retired,
    Dead,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ZoneTimingParameters {
    pub zone_ttl: Duration,
    pub propagation_delay: Duration,
    pub parent_ds_ttl: Duration,
    pub parent_propagation_delay: Duration,
    pub max_zone_ttl: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RolloverAction {
    PublishNewKey,
    ActivateNewKey,
    PublishNewDs,
    RetireOldKey,
    RemoveOldDs,
    RemoveOldKey,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RolloverStep {
    pub action: RolloverAction,
    /// Delay to wait *before* executing this action, relative to the previous step.
    pub wait_before: Duration,
}

#[derive(Debug, Clone)]
pub struct DnssecRolloverPlanner {
    timing: ZoneTimingParameters,
}

impl DnssecRolloverPlanner {
    pub fn new(timing: ZoneTimingParameters) -> Self {
        Self { timing }
    }

    /// Validates a key phase transition
    pub fn validate_transition(from: KeyPhase, to: KeyPhase) -> Result<(), RolloverError> {
        let valid = match (from, to) {
            (KeyPhase::Generated, KeyPhase::Published) => true,
            (KeyPhase::Published, KeyPhase::Active) => true,
            (KeyPhase::Active, KeyPhase::Retired) => true,
            (KeyPhase::Retired, KeyPhase::Dead) => true,
            _ => false,
        };
        if valid {
            Ok(())
        } else {
            Err(RolloverError::InvalidTransition { from, to })
        }
    }

    /// Generate a schedule of actions for a ZSK or KSK rollover.
    pub fn generate_schedule(
        &self,
        key_type: KeyType,
        method: RolloverMethod,
    ) -> Result<Vec<RolloverStep>, RolloverError> {
        match (key_type, method) {
            (KeyType::ZSK, RolloverMethod::PrePublish) => Ok(self.zsk_pre_publish_schedule()),
            (KeyType::ZSK, RolloverMethod::DoubleSignature) => Ok(self.zsk_double_signature_schedule()),
            (KeyType::KSK, RolloverMethod::DoubleDS) => Ok(self.ksk_double_ds_schedule()),
            (KeyType::KSK, RolloverMethod::DoubleSignature) => Ok(self.ksk_double_signature_schedule()),
            (kt, m) => Err(RolloverError::UnsupportedMethod(m, kt)),
        }
    }

    /// ZSK Pre-Publish schedule (RFC 6781 / RFC 7583)
    fn zsk_pre_publish_schedule(&self) -> Vec<RolloverStep> {
        vec![
            RolloverStep {
                action: RolloverAction::PublishNewKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::ActivateNewKey,
                wait_before: self.timing.propagation_delay + self.timing.zone_ttl,
            },
            RolloverStep {
                action: RolloverAction::RetireOldKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::RemoveOldKey,
                wait_before: self.timing.propagation_delay + self.timing.max_zone_ttl,
            },
        ]
    }

    /// ZSK Double-Signature schedule
    fn zsk_double_signature_schedule(&self) -> Vec<RolloverStep> {
        vec![
            RolloverStep {
                action: RolloverAction::PublishNewKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::ActivateNewKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::RetireOldKey,
                wait_before: self.timing.propagation_delay + self.timing.zone_ttl,
            },
            RolloverStep {
                action: RolloverAction::RemoveOldKey,
                wait_before: self.timing.propagation_delay + self.timing.max_zone_ttl,
            },
        ]
    }

    /// KSK Double-DS schedule (RFC 6781 / RFC 7583)
    fn ksk_double_ds_schedule(&self) -> Vec<RolloverStep> {
        vec![
            RolloverStep {
                action: RolloverAction::PublishNewKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::PublishNewDs,
                wait_before: self.timing.propagation_delay + self.timing.zone_ttl,
            },
            RolloverStep {
                action: RolloverAction::ActivateNewKey,
                wait_before: self.timing.parent_propagation_delay + self.timing.parent_ds_ttl,
            },
            RolloverStep {
                action: RolloverAction::RetireOldKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::RemoveOldDs,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::RemoveOldKey,
                wait_before: self.timing.parent_propagation_delay + self.timing.parent_ds_ttl,
            },
        ]
    }

    /// KSK Double-Signature schedule
    fn ksk_double_signature_schedule(&self) -> Vec<RolloverStep> {
        vec![
            RolloverStep {
                action: RolloverAction::PublishNewKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::ActivateNewKey,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::PublishNewDs,
                wait_before: self.timing.propagation_delay + self.timing.zone_ttl,
            },
            RolloverStep {
                action: RolloverAction::RemoveOldDs,
                wait_before: Duration::ZERO,
            },
            RolloverStep {
                action: RolloverAction::RetireOldKey,
                wait_before: self.timing.parent_propagation_delay + self.timing.parent_ds_ttl,
            },
            RolloverStep {
                action: RolloverAction::RemoveOldKey,
                wait_before: Duration::ZERO,
            },
        ]
    }
}

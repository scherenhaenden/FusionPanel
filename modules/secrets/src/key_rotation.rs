#![forbid(unsafe_code)]

use crate::envelope_encryption::{EnvelopeKeyManager, Kek, WrappedDek, Error as EnvelopeError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RotationStatus {
    Pending,
    InProgress,
    Completed,
    Failed(String),
}

#[derive(Debug)]
pub enum RotationError {
    InvalidState(&'static str),
    CryptoError(EnvelopeError),
    RetirementGateFailed(&'static str),
}

impl std::error::Error for RotationError {}

impl std::fmt::Display for RotationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RotationError::InvalidState(msg) => write!(f, "Invalid state: {}", msg),
            RotationError::CryptoError(e) => write!(f, "Crypto error: {}", e),
            RotationError::RetirementGateFailed(msg) => write!(f, "Retirement gate failed: {}", msg),
        }
    }
}

pub struct KeyRotationPlanner {
    pub current_version: u32,
    pub target_version: u32,
    pub status: RotationStatus,
}

impl KeyRotationPlanner {
    pub fn new(current_version: u32) -> Self {
        Self {
            current_version,
            target_version: current_version + 1,
            status: RotationStatus::Pending,
        }
    }

    pub fn begin_rotation(&mut self) -> Result<(), RotationError> {
        if self.status != RotationStatus::Pending {
            return Err(RotationError::InvalidState("Rotation can only begin from Pending state"));
        }
        self.status = RotationStatus::InProgress;
        Ok(())
    }

    pub fn reencrypt_wrapped_dek(
        &self,
        old_kek: &Kek,
        new_kek: &Kek,
        wrapped_dek: &WrappedDek,
    ) -> Result<WrappedDek, RotationError> {
        if self.status != RotationStatus::InProgress {
            return Err(RotationError::InvalidState("Rotation must be InProgress to re-encrypt DEKs"));
        }
        EnvelopeKeyManager::rotate_kek(old_kek, new_kek, wrapped_dek)
            .map_err(RotationError::CryptoError)
    }

    pub fn mark_completed(&mut self) -> Result<(), RotationError> {
        if self.status != RotationStatus::InProgress {
            return Err(RotationError::InvalidState("Only InProgress rotations can be completed"));
        }
        self.status = RotationStatus::Completed;
        Ok(())
    }

    pub fn mark_failed(&mut self, reason: String) {
        self.status = RotationStatus::Failed(reason);
    }

    pub fn enforce_retirement_gate(&self, requested_version: u32) -> Result<(), RotationError> {
        let active_version = if self.status == RotationStatus::Completed {
            self.target_version
        } else {
            self.current_version
        };

        if requested_version < active_version {
            Err(RotationError::RetirementGateFailed(
                "Requested key version is retired and blocked by retirement gate",
            ))
        } else {
            Ok(())
        }
    }
}

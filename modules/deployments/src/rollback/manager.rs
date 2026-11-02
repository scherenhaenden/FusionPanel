use std::sync::Arc;
use std::sync::Mutex;
use thiserror::Error;
use super::inventory::{ReleaseInventory, Release, ObservedState, ReleaseStatus};
use chrono::Utc;

#[derive(Debug, Error)]
pub enum RollbackError {
    #[error("Release inventory error: {0}")]
    InventoryError(#[from] super::inventory::InventoryError),
    #[error("No previous healthy release found for target {0}")]
    NoHealthyRelease(String),
}

pub struct RollbackManager {
    inventory: Arc<Mutex<ReleaseInventory>>,
}

impl RollbackManager {
    pub fn new(inventory: Arc<Mutex<ReleaseInventory>>) -> Self {
        Self { inventory }
    }

    pub fn rollback_to_last_healthy(&self, target_id: &str) -> Result<Release, RollbackError> {
        let mut inventory = self.inventory.lock().unwrap();

        let previous = inventory.get_last_healthy_release(target_id);
        if let Some(healthy_release) = previous {
            // Find the currently active release (the one that presumably failed and we are rolling back FROM)
            let active = inventory.get_active_release(target_id);
            if let Some(active_rel) = active {
                if active_rel.id != healthy_release.id {
                    let new_state = ObservedState {
                        status: ReleaseStatus::RolledBack,
                        last_checked_at: Utc::now(),
                    };
                    inventory.update_observed_state(target_id, active_rel.id, new_state)?;
                }
            }

            // Re-deploy or mark the healthy release as active
            let new_state = ObservedState {
                status: ReleaseStatus::Active,
                last_checked_at: Utc::now(),
            };
            inventory.update_observed_state(target_id, healthy_release.id, new_state.clone())?;

            let mut updated_release = healthy_release.clone();
            updated_release.observed_state = new_state;
            Ok(updated_release)
        } else {
            Err(RollbackError::NoHealthyRelease(target_id.to_string()))
        }
    }
}

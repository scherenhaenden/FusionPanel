use std::collections::{HashMap, VecDeque};
use serde::{Serialize, Deserialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ReleaseStatus {
    Pending,
    Healthy,
    Failed,
    RolledBack,
    Active,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Plan {
    pub target_id: String,
    pub revision: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppliedResult {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservedState {
    pub status: ReleaseStatus,
    pub last_checked_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Release {
    pub id: Uuid,
    pub plan: Plan,
    pub applied_result: Option<AppliedResult>,
    pub observed_state: ObservedState,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum InventoryError {
    #[error("Release not found")]
    NotFound,
    #[error("Invalid state transition")]
    InvalidTransition,
}

pub struct ReleaseInventory {
    releases_by_target: HashMap<String, VecDeque<Release>>,
    max_healthy_retention: usize,
}

impl ReleaseInventory {
    pub fn new(max_healthy_retention: usize) -> Self {
        Self {
            releases_by_target: HashMap::new(),
            max_healthy_retention,
        }
    }

    pub fn add_release(&mut self, release: Release) {
        let target_releases = self.releases_by_target
            .entry(release.plan.target_id.clone())
            .or_default();

        target_releases.push_front(release);

        let mut healthy_count = 0;
        let mut indices_to_remove = Vec::new();
        for (i, r) in target_releases.iter().enumerate() {
            if r.observed_state.status == ReleaseStatus::Healthy || r.observed_state.status == ReleaseStatus::Active {
                healthy_count += 1;
                if healthy_count > self.max_healthy_retention {
                    indices_to_remove.push(i);
                }
            }
        }

        for i in indices_to_remove.into_iter().rev() {
            target_releases.remove(i);
        }
    }

    pub fn get_last_healthy_release(&self, target_id: &str) -> Option<Release> {
        if let Some(releases) = self.releases_by_target.get(target_id) {
            // Because we push_front, the first Healthy one we find that is NOT the currently Active one
            // is the *previous* healthy release. Wait, if Active is the one that failed, it might be Active but broken.
            // A healthy release is just status == Healthy. Let's just find the first Healthy.
            for release in releases {
                if release.observed_state.status == ReleaseStatus::Healthy {
                    return Some(release.clone());
                }
            }
        }
        None
    }

    pub fn get_active_release(&self, target_id: &str) -> Option<Release> {
        if let Some(releases) = self.releases_by_target.get(target_id) {
            for release in releases {
                if release.observed_state.status == ReleaseStatus::Active {
                    return Some(release.clone());
                }
            }
        }
        None
    }

    pub fn get_release(&self, target_id: &str, release_id: Uuid) -> Option<Release> {
        self.releases_by_target
            .get(target_id)
            .and_then(|releases| releases.iter().find(|r| r.id == release_id).cloned())
    }

    pub fn update_observed_state(
        &mut self,
        target_id: &str,
        release_id: Uuid,
        new_state: ObservedState,
    ) -> Result<(), InventoryError> {
        if let Some(releases) = self.releases_by_target.get_mut(target_id) {
            if let Some(release) = releases.iter_mut().find(|r| r.id == release_id) {
                release.observed_state = new_state;
                return Ok(());
            }
        }
        Err(InventoryError::NotFound)
    }
}

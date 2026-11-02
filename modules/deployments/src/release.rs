use chrono::Utc;
use std::fs;
use std::os::unix::fs::symlink;
use std::path::PathBuf;

use crate::error::DeploymentError;
use crate::types::{AppliedResult, BlueGreenSlot, DeploymentId, DeploymentState, ReleaseId};

/// Directory layout for a site's releases:
/// - `<base_dir>/releases/<release_id>/`
/// - `<base_dir>/current` (symlink to active release)
/// - `<base_dir>/slots/blue` & `<base_dir>/slots/green` (for blue/green mode)
/// - `<base_dir>/active_slot` (contains "blue" or "green")
#[derive(Debug, Clone)]
pub struct ReleaseManager {
    base_dir: PathBuf,
}

impl ReleaseManager {
    pub fn new(base_dir: impl Into<PathBuf>) -> Self {
        Self {
            base_dir: base_dir.into(),
        }
    }

    pub fn releases_dir(&self) -> PathBuf {
        self.base_dir.join("releases")
    }

    pub fn current_symlink(&self) -> PathBuf {
        self.base_dir.join("current")
    }

    pub fn slots_dir(&self) -> PathBuf {
        self.base_dir.join("slots")
    }

    pub fn active_slot_file(&self) -> PathBuf {
        self.base_dir.join("active_slot")
    }

    pub fn initialize(&self) -> Result<(), DeploymentError> {
        fs::create_dir_all(self.releases_dir()).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to create releases dir: {e}"))
        })?;
        fs::create_dir_all(self.slots_dir()).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to create slots dir: {e}"))
        })?;
        Ok(())
    }

    /// Prepares an isolated release directory for an upcoming release ID
    pub fn prepare_release_dir(&self, release_id: &ReleaseId) -> Result<PathBuf, DeploymentError> {
        self.initialize()?;
        let rel_dir = self.releases_dir().join(&release_id.0);
        if rel_dir.exists() {
            return Err(DeploymentError::Conflict(format!(
                "release directory already exists for {}",
                release_id.0
            )));
        }
        fs::create_dir_all(&rel_dir).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to create release dir: {e}"))
        })?;
        Ok(rel_dir)
    }

    /// Atomically points `<base_dir>/current` to `releases/<release_id>`
    pub fn activate_directory_switch(
        &self,
        deployment_id: &DeploymentId,
        release_id: &ReleaseId,
    ) -> Result<AppliedResult, DeploymentError> {
        let target_dir = self.releases_dir().join(&release_id.0);
        if !target_dir.exists() {
            return Err(DeploymentError::NotFound(format!(
                "release {} does not exist in releases directory",
                release_id.0
            )));
        }

        let current_link = self.current_symlink();
        let previous_release_id = self.current_active_release()?;

        // Atomic symlink switch using temporary symlink and rename
        let tmp_link = self
            .base_dir
            .join(format!(".current_tmp_{}", uuid::Uuid::new_v4()));
        symlink(&target_dir, &tmp_link).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to create temporary symlink: {e}"))
        })?;

        fs::rename(&tmp_link, &current_link).map_err(|e| {
            let _ = fs::remove_file(&tmp_link);
            DeploymentError::OperationFailed(format!("failed to atomically replace symlink: {e}"))
        })?;

        Ok(AppliedResult {
            deployment_id: deployment_id.clone(),
            release_id: Some(release_id.clone()),
            previous_release_id,
            state: DeploymentState::Active,
            active_path: Some(target_dir.display().to_string()),
            slot: None,
            applied_at: Utc::now(),
        })
    }

    /// Rolls back `<base_dir>/current` to `target_release_id` (or previous if unspecified)
    pub fn rollback_directory_switch(
        &self,
        deployment_id: &DeploymentId,
        target_release: Option<&ReleaseId>,
    ) -> Result<AppliedResult, DeploymentError> {
        let target = match target_release {
            Some(r) => r.clone(),
            None => {
                // Find previous release from list of sorted releases
                let current = self.current_active_release()?;
                let list = self.list_releases()?;
                let mut prior = None;
                for (i, r) in list.iter().enumerate() {
                    if Some(r) == current.as_ref() && i > 0 {
                        prior = Some(list[i - 1].clone());
                        break;
                    }
                }
                prior.ok_or_else(|| {
                    DeploymentError::InvalidInput(
                        "no prior release found to roll back to".to_string(),
                    )
                })?
            }
        };

        let res = self.activate_directory_switch(deployment_id, &target)?;
        Ok(AppliedResult {
            state: DeploymentState::RolledBack,
            ..res
        })
    }

    /// Inspects current symlink destination to find active ReleaseId
    pub fn current_active_release(&self) -> Result<Option<ReleaseId>, DeploymentError> {
        let current_link = self.current_symlink();
        if !current_link.exists() {
            return Ok(None);
        }
        let target = fs::read_link(&current_link).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to read current symlink: {e}"))
        })?;

        let rel_name = target.file_name().and_then(|f| f.to_str()).ok_or_else(|| {
            DeploymentError::InvalidInput("invalid release path symlink".to_string())
        })?;

        Ok(Some(ReleaseId(rel_name.to_string())))
    }

    /// Lists all existing release IDs in chronological/directory order
    pub fn list_releases(&self) -> Result<Vec<ReleaseId>, DeploymentError> {
        let rel_dir = self.releases_dir();
        if !rel_dir.exists() {
            return Ok(Vec::new());
        }
        let mut entries: Vec<(std::time::SystemTime, ReleaseId)> = Vec::new();
        for entry in fs::read_dir(rel_dir).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to read releases dir: {e}"))
        })? {
            let entry = entry.map_err(|e| DeploymentError::OperationFailed(e.to_string()))?;
            let path = entry.path();
            if path.is_dir() {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    let meta = entry.metadata().ok();
                    let created = meta
                        .and_then(|m| m.created().or_else(|_| m.modified()).ok())
                        .unwrap_or(std::time::UNIX_EPOCH);
                    entries.push((created, ReleaseId(name.to_string())));
                }
            }
        }
        entries.sort_by_key(|(t, _)| *t);
        Ok(entries.into_iter().map(|(_, r)| r).collect())
    }

    /// Prunes old releases, keeping the latest `keep_count` releases, ensuring the active release is NEVER removed.
    pub fn prune_old_releases(&self, keep_count: usize) -> Result<Vec<ReleaseId>, DeploymentError> {
        let releases = self.list_releases()?;
        if releases.len() <= keep_count {
            return Ok(Vec::new());
        }

        let active = self.current_active_release()?;
        let to_prune_candidates = &releases[..releases.len() - keep_count];
        let mut pruned = Vec::new();

        for rel in to_prune_candidates {
            if Some(rel) == active.as_ref() {
                continue; // Invariant: never delete active release
            }
            let path = self.releases_dir().join(&rel.0);
            if path.exists() {
                fs::remove_dir_all(&path).map_err(|e| {
                    DeploymentError::OperationFailed(format!(
                        "failed to prune release {}: {e}",
                        rel.0
                    ))
                })?;
                pruned.push(rel.clone());
            }
        }

        Ok(pruned)
    }

    // --- Blue-Green deployment mode methods ---

    /// Gets active slot (defaults to Blue if not set)
    pub fn get_active_slot(&self) -> Result<BlueGreenSlot, DeploymentError> {
        let file = self.active_slot_file();
        if !file.exists() {
            return Ok(BlueGreenSlot::Blue);
        }
        let content = fs::read_to_string(&file).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to read active slot file: {e}"))
        })?;
        match content.trim() {
            "green" => Ok(BlueGreenSlot::Green),
            _ => Ok(BlueGreenSlot::Blue),
        }
    }

    /// Sets up target slot directory for upcoming blue-green deployment
    pub fn prepare_slot_dir(&self, slot: BlueGreenSlot) -> Result<PathBuf, DeploymentError> {
        self.initialize()?;
        let slot_name = match slot {
            BlueGreenSlot::Blue => "blue",
            BlueGreenSlot::Green => "green",
        };
        let slot_path = self.slots_dir().join(slot_name);
        if slot_path.exists() {
            fs::remove_dir_all(&slot_path).map_err(|e| {
                DeploymentError::OperationFailed(format!("failed to clean slot dir: {e}"))
            })?;
        }
        fs::create_dir_all(&slot_path).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to create slot dir: {e}"))
        })?;
        Ok(slot_path)
    }

    /// Swaps the active slot to `target_slot` after health verification
    pub fn switch_active_slot(
        &self,
        deployment_id: &DeploymentId,
        target_slot: BlueGreenSlot,
        health_verified: bool,
    ) -> Result<AppliedResult, DeploymentError> {
        if !health_verified {
            return Err(DeploymentError::OperationFailed(
                "cannot cut over traffic to unverified slot".to_string(),
            ));
        }

        let slot_name = match target_slot {
            BlueGreenSlot::Blue => "blue",
            BlueGreenSlot::Green => "green",
        };

        let tmp_file = self
            .base_dir
            .join(format!(".slot_tmp_{}", uuid::Uuid::new_v4()));
        fs::write(&tmp_file, slot_name).map_err(|e| {
            DeploymentError::OperationFailed(format!("failed to write tmp slot file: {e}"))
        })?;

        fs::rename(&tmp_file, self.active_slot_file()).map_err(|e| {
            let _ = fs::remove_file(&tmp_file);
            DeploymentError::OperationFailed(format!(
                "failed to atomically replace active slot file: {e}"
            ))
        })?;

        let slot_path = self.slots_dir().join(slot_name);

        Ok(AppliedResult {
            deployment_id: deployment_id.clone(),
            release_id: None,
            previous_release_id: None,
            state: DeploymentState::Active,
            active_path: Some(slot_path.display().to_string()),
            slot: Some(target_slot),
            applied_at: Utc::now(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_directory_switch_and_rollback() {
        let temp = TempDir::new().unwrap();
        let mgr = ReleaseManager::new(temp.path());

        let rel1 = ReleaseId("release-1".to_string());
        let rel2 = ReleaseId("release-2".to_string());

        let dir1 = mgr.prepare_release_dir(&rel1).unwrap();
        fs::write(dir1.join("app.txt"), b"Version 1.0").unwrap();

        let dir2 = mgr.prepare_release_dir(&rel2).unwrap();
        fs::write(dir2.join("app.txt"), b"Version 2.0").unwrap();

        let dep1 = DeploymentId::new();
        let dep2 = DeploymentId::new();

        // Activate release 1
        let res1 = mgr.activate_directory_switch(&dep1, &rel1).unwrap();
        assert_eq!(res1.state, DeploymentState::Active);
        assert_eq!(mgr.current_active_release().unwrap(), Some(rel1.clone()));

        let active_content1 = fs::read_to_string(mgr.current_symlink().join("app.txt")).unwrap();
        assert_eq!(active_content1, "Version 1.0");

        // Activate release 2
        let res2 = mgr.activate_directory_switch(&dep2, &rel2).unwrap();
        assert_eq!(res2.previous_release_id, Some(rel1.clone()));

        let active_content2 = fs::read_to_string(mgr.current_symlink().join("app.txt")).unwrap();
        assert_eq!(active_content2, "Version 2.0");

        // Roll back to previous
        let roll_dep = DeploymentId::new();
        let roll_res = mgr
            .rollback_directory_switch(&roll_dep, Some(&rel1))
            .unwrap();
        assert_eq!(roll_res.state, DeploymentState::RolledBack);
        assert_eq!(mgr.current_active_release().unwrap(), Some(rel1));

        let active_content_after_rollback =
            fs::read_to_string(mgr.current_symlink().join("app.txt")).unwrap();
        assert_eq!(active_content_after_rollback, "Version 1.0");
    }

    #[test]
    fn test_blue_green_slot_switch() {
        let temp = TempDir::new().unwrap();
        let mgr = ReleaseManager::new(temp.path());

        assert_eq!(mgr.get_active_slot().unwrap(), BlueGreenSlot::Blue);

        let green_dir = mgr.prepare_slot_dir(BlueGreenSlot::Green).unwrap();
        fs::write(green_dir.join("index.html"), b"Green slot").unwrap();

        let dep = DeploymentId::new();
        // Fails if health_verified is false
        let unverified = mgr.switch_active_slot(&dep, BlueGreenSlot::Green, false);
        assert!(unverified.is_err());

        // Succeeds when health_verified is true
        let verified = mgr
            .switch_active_slot(&dep, BlueGreenSlot::Green, true)
            .unwrap();
        assert_eq!(verified.slot, Some(BlueGreenSlot::Green));
        assert_eq!(mgr.get_active_slot().unwrap(), BlueGreenSlot::Green);
    }

    #[test]
    fn test_prune_never_deletes_active_release() {
        let temp = TempDir::new().unwrap();
        let mgr = ReleaseManager::new(temp.path());

        let rel1 = ReleaseId("rel-1".to_string());
        let rel2 = ReleaseId("rel-2".to_string());
        let rel3 = ReleaseId("rel-3".to_string());

        let d1 = mgr.prepare_release_dir(&rel1).unwrap();
        fs::write(d1.join("file"), b"1").unwrap();
        let d2 = mgr.prepare_release_dir(&rel2).unwrap();
        fs::write(d2.join("file"), b"2").unwrap();
        let d3 = mgr.prepare_release_dir(&rel3).unwrap();
        fs::write(d3.join("file"), b"3").unwrap();

        // rel1 is active
        let dep = DeploymentId::new();
        mgr.activate_directory_switch(&dep, &rel1).unwrap();

        // Prune keeping only 1 newest
        let _ = mgr.prune_old_releases(1).unwrap();

        // rel1 must still exist because it is active
        assert!(d1.exists());
    }
}

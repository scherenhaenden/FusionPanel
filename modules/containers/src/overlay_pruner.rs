#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PrunePlan {
    pub reclaimable_bytes: u64,
    pub orphaned_overlay_dirs: Vec<PathBuf>,
    pub orphaned_shm_dirs: Vec<PathBuf>,
}

pub struct OverlayPruner {
    pub overlay2_dir: PathBuf,
    pub shm_dir: PathBuf,
}

impl OverlayPruner {
    pub fn new<P1: AsRef<Path>, P2: AsRef<Path>>(overlay2_dir: P1, shm_dir: P2) -> Self {
        Self {
            overlay2_dir: overlay2_dir.as_ref().to_path_buf(),
            shm_dir: shm_dir.as_ref().to_path_buf(),
        }
    }

    pub fn plan_prune(
        &self,
        active_overlay_layers: &HashSet<String>,
        active_shm_mounts: &HashSet<String>,
    ) -> std::io::Result<PrunePlan> {
        let mut plan = PrunePlan::default();

        // Audit overlay2_dir
        if self.overlay2_dir.exists() && self.overlay2_dir.is_dir() {
            for entry in fs::read_dir(&self.overlay2_dir)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy().to_string();

                // Skip special directories or files like 'l' and 'backingFsBlockDev'
                if name == "l" || name == "backingFsBlockDev" {
                    continue;
                }

                if !active_overlay_layers.contains(&name) {
                    let path = entry.path();
                    let meta = fs::symlink_metadata(&path)?;
                    if meta.is_dir() {
                        let size = get_dir_size(&path)?;
                        plan.reclaimable_bytes += size;
                        plan.orphaned_overlay_dirs.push(path);
                    }
                }
            }
        }

        // Audit shm_dir
        if self.shm_dir.exists() && self.shm_dir.is_dir() {
            for entry in fs::read_dir(&self.shm_dir)? {
                let entry = entry?;
                let file_name = entry.file_name();
                let name = file_name.to_string_lossy().to_string();

                if !active_shm_mounts.contains(&name) {
                    let path = entry.path();
                    let meta = fs::symlink_metadata(&path)?;
                    if meta.is_dir() {
                        let size = get_dir_size(&path)?;
                        plan.reclaimable_bytes += size;
                        plan.orphaned_shm_dirs.push(path);
                    }
                }
            }
        }

        plan.orphaned_overlay_dirs.sort();
        plan.orphaned_shm_dirs.sort();

        Ok(plan)
    }
}

fn get_dir_size(path: &Path) -> std::io::Result<u64> {
    let meta = fs::symlink_metadata(path)?;
    let mut total_size = meta.len();

    if meta.is_dir() {
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            total_size += get_dir_size(&entry.path())?;
        }
    }

    Ok(total_size)
}

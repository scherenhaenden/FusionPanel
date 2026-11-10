#![forbid(unsafe_code)]

use std::collections::{HashMap, HashSet};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SnapshotMode {
    ReadOnlyBind,
    PreFreezeCoordination,
}

#[derive(Debug, Clone)]
pub struct SnapshotPlan {
    pub volume_name: String,
    pub source_path: PathBuf,
    pub mode: SnapshotMode,
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub size: u64,
    pub modified: SystemTime,
}

#[derive(Debug, Clone, Default)]
pub struct VolumeManifest {
    pub files: HashMap<PathBuf, FileInfo>, // Relative paths
}

#[derive(Debug, Clone)]
pub struct IncrementalBackupResult {
    pub files_added: Vec<PathBuf>,
    pub files_modified: Vec<PathBuf>,
    pub files_removed: Vec<PathBuf>,
    pub size_diff: i64,
}

pub struct VolumeCloner;

impl VolumeCloner {
    pub fn plan_snapshot(
        volume_name: &str,
        source_path: &Path,
        mode: SnapshotMode,
    ) -> SnapshotPlan {
        SnapshotPlan {
            volume_name: volume_name.to_string(),
            source_path: source_path.to_path_buf(),
            mode,
        }
    }

    pub fn generate_manifest(source_path: &Path) -> io::Result<VolumeManifest> {
        let mut manifest = VolumeManifest::default();
        Self::traverse_and_build_manifest(source_path, source_path, &mut manifest)?;
        Ok(manifest)
    }

    fn traverse_and_build_manifest(
        root: &Path,
        current: &Path,
        manifest: &mut VolumeManifest,
    ) -> io::Result<()> {
        if !current.is_dir() {
            return Ok(());
        }

        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;

            if metadata.is_dir() {
                Self::traverse_and_build_manifest(root, &path, manifest)?;
            } else if metadata.is_file() {
                let rel_path = path
                    .strip_prefix(root)
                    .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?
                    .to_path_buf();

                manifest.files.insert(
                    rel_path,
                    FileInfo {
                        size: metadata.len(),
                        modified: metadata.modified()?,
                    },
                );
            }
        }
        Ok(())
    }

    pub fn calculate_difference(
        old_manifest: &VolumeManifest,
        new_manifest: &VolumeManifest,
    ) -> IncrementalBackupResult {
        let mut files_added = Vec::new();
        let mut files_modified = Vec::new();
        let mut files_removed = Vec::new();
        let mut size_diff: i64 = 0;

        let old_keys: HashSet<_> = old_manifest.files.keys().collect();
        let new_keys: HashSet<_> = new_manifest.files.keys().collect();

        // Check for removed files
        for &key in old_keys.difference(&new_keys) {
            files_removed.push(key.clone());
            size_diff -= old_manifest.files.get(key).unwrap().size as i64;
        }

        // Check for added or modified files
        for (path, new_info) in &new_manifest.files {
            if let Some(old_info) = old_manifest.files.get(path) {
                // Simplified modified check based on size or modified time (could be hash based)
                if old_info.size != new_info.size || old_info.modified != new_info.modified {
                    files_modified.push(path.clone());
                    size_diff += (new_info.size as i64) - (old_info.size as i64);
                }
            } else {
                files_added.push(path.clone());
                size_diff += new_info.size as i64;
            }
        }

        IncrementalBackupResult {
            files_added,
            files_modified,
            files_removed,
            size_diff,
        }
    }

    pub fn create_cow_clone(source_path: &Path, dest_path: &Path) -> io::Result<()> {
        if !dest_path.exists() {
            fs::create_dir_all(dest_path)?;
        }

        Self::cow_traverse_and_link(source_path, source_path, dest_path)
    }

    fn cow_traverse_and_link(root: &Path, current: &Path, dest_root: &Path) -> io::Result<()> {
        for entry in fs::read_dir(current)? {
            let entry = entry?;
            let path = entry.path();
            let metadata = entry.metadata()?;

            let rel_path = path
                .strip_prefix(root)
                .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            let dest_file_path = dest_root.join(rel_path);

            if metadata.is_dir() {
                if !dest_file_path.exists() {
                    fs::create_dir_all(&dest_file_path)?;
                }
                Self::cow_traverse_and_link(root, &path, dest_root)?;
            } else if metadata.is_file() {
                if dest_file_path.exists() {
                    fs::remove_file(&dest_file_path)?;
                }
                fs::hard_link(&path, &dest_file_path)?;
            }
        }
        Ok(())
    }
}

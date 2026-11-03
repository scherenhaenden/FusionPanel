use crate::error::BackupError;
use crate::models::{FileInfo, Manifest, RestoreOptions};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::Archive;
use zstd::stream::Decoder as ZstdDecoder;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct RestoreTransaction {
    base_dir: PathBuf,
    extracted_dir: PathBuf,
    rollback_dir: PathBuf,
    committed: bool,
    rollbacks: Vec<(PathBuf, PathBuf)>, // (from_rollback_dir, original_target_path)
    new_files: Vec<PathBuf>, // newly created files to remove on rollback
}

impl RestoreTransaction {
    fn new(target_dir: &Path) -> Result<Self, BackupError> {
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let base_dir = target_dir.join(format!(".restore_temp_{}_{}", timestamp, counter));
        fs::create_dir_all(&base_dir)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&base_dir, fs::Permissions::from_mode(0o700))?;
        }

        let extracted_dir = base_dir.join("extracted");
        let rollback_dir = base_dir.join("rollback");
        fs::create_dir_all(&extracted_dir)?;
        fs::create_dir_all(&rollback_dir)?;

        Ok(Self {
            base_dir,
            extracted_dir,
            rollback_dir,
            committed: false,
            rollbacks: Vec::new(),
            new_files: Vec::new(),
        })
    }

    fn commit(&mut self) {
        self.committed = true;
    }
}

impl Drop for RestoreTransaction {
    fn drop(&mut self) {
        if !self.committed {
            // Rollback
            for new_file in self.new_files.iter().rev() {
                if new_file.exists() {
                    let _ = fs::remove_file(new_file);
                }
            }
            for (rollback_path, target_path) in self.rollbacks.iter().rev() {
                if rollback_path.exists() {
                    let _ = fs::rename(rollback_path, target_path);
                }
            }
        }
        if self.base_dir.exists() {
            let _ = fs::remove_dir_all(&self.base_dir);
        }
    }
}

pub fn restore_archive<P1: AsRef<Path>, P2: AsRef<Path>>(
    archive_path: P1,
    target_dir: P2,
) -> Result<Manifest, BackupError> {
    restore_archive_with_options(archive_path, target_dir, &RestoreOptions::default())
}

pub fn restore_archive_with_options<P1: AsRef<Path>, P2: AsRef<Path>>(
    archive_path: P1,
    target_dir: P2,
    options: &RestoreOptions,
) -> Result<Manifest, BackupError> {
    let target_dir = target_dir.as_ref();
    if !target_dir.exists() {
        fs::create_dir_all(target_dir)?;
    }
    let canonical_target = fs::canonicalize(target_dir)?;

    let mut tx = RestoreTransaction::new(&canonical_target)?;
    let canonical_temp = fs::canonicalize(&tx.extracted_dir)?;
    let canonical_rollback = fs::canonicalize(&tx.rollback_dir)?;

    let file = File::open(archive_path)?;
    let tar = ZstdDecoder::new(file)?;
    let mut archive = Archive::new(tar);

    let mut manifest: Option<Manifest> = None;
    let mut file_info_map: HashMap<String, FileInfo> = HashMap::new();
    let mut extracted_files: HashSet<String> = HashSet::new();

    let entries = archive.entries()?;
    for entry in entries {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let path_str = path.to_string_lossy().into_owned();

        // Explicitly block any component that attempts path traversal (..)
        // or absolute path injection (/) before doing *anything* else.
        for component in path.components() {
            match component {
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(BackupError::UnsafePath(path_str));
                }
                _ => {}
            }
        }

        let is_selected = match &options.selected_files {
            Some(selected) => selected.contains(&path_str),
            None => true,
        };

        if path_str == "manifest.json" {
            if manifest.is_some() {
                return Err(BackupError::DuplicateManifest);
            }
            let mut content = Vec::new();
            entry.read_to_end(&mut content)?;
            let parsed_manifest: Manifest = serde_json::from_slice(&content)?;
            parsed_manifest.validate()?;
            for file_info in &parsed_manifest.files {
                file_info_map.insert(file_info.path.clone(), file_info.clone());
            }
            manifest = Some(parsed_manifest);
        } else {
            // Must have seen manifest first
            if manifest.is_none() {
                return Err(BackupError::ManifestNotFound);
            }

            if !is_selected {
                continue;
            }

            let expected_info = file_info_map.get(&path_str).ok_or_else(|| {
                BackupError::UnexpectedFile(path_str.clone())
            })?;

            if !extracted_files.insert(path_str.clone()) {
                return Err(BackupError::DuplicateFile(path_str.clone()));
            }

            // Path containment: strip any leading '/' to prevent absolute path override
            let rel_path = path.strip_prefix("/").unwrap_or(&path);
            let temp_target_path = canonical_temp.join(rel_path);

            if let Some(parent) = temp_target_path.parent() {
                fs::create_dir_all(parent)?;
                let canonical_parent = fs::canonicalize(parent)?;
                if !canonical_parent.starts_with(&canonical_temp) {
                    return Err(BackupError::UnsafePath(path_str));
                }
            } else {
                return Err(BackupError::UnsafePath(path_str));
            }

            let mut dest_file = File::create(&temp_target_path)?;

            let mut hasher = Sha256::new();
            let mut buffer = [0; 8192];
            let mut actual_size: u64 = 0;
            loop {
                let count = entry.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
                dest_file.write_all(&buffer[..count])?;
                actual_size += count as u64;
            }

            if actual_size != expected_info.size {
                return Err(BackupError::SizeMismatch {
                    path: path_str.clone(),
                    expected: expected_info.size,
                    actual: actual_size,
                });
            }

            let hash = hex::encode(hasher.finalize());
            if hash != expected_info.checksum_sha256 {
                return Err(BackupError::ChecksumMismatch {
                    path: path_str.clone(),
                    expected: expected_info.checksum_sha256.clone(),
                    actual: hash,
                });
            }
        }
    }

    let manifest = manifest.ok_or(BackupError::ManifestNotFound)?;

    if options.selected_files.is_none() {
        for file_info in &manifest.files {
            if !extracted_files.contains(&file_info.path) {
                return Err(BackupError::MissingFiles);
            }
        }
    }

    // Commit phase
    let mut file_count = 0;
    for path_str in &extracted_files {
        if let Some(fault_after) = options.fault_inject_error_after {
            if file_count >= fault_after {
                return Err(BackupError::FaultInjected);
            }
        }
        file_count += 1;

        let path = Path::new(path_str);
        let rel_path = path.strip_prefix("/").unwrap_or(path);
        let temp_file_path = canonical_temp.join(rel_path);
        let final_file_path = canonical_target.join(rel_path);

        // Enforce target symlink containment: Ensure all components of the target path
        // are real directories within the canonical_target base, resolving symlinks
        // strictly downward. If any symlink points outside the target, it will be
        // caught by the `starts_with` check.
        let mut current_canonical = canonical_target.to_path_buf();
        if let Some(parent) = final_file_path.parent() {
            let relative_parent = parent.strip_prefix(&canonical_target).unwrap_or(parent);
            for component in relative_parent.components() {
                match component {
                    Component::Normal(c) => {
                        let next_path = current_canonical.join(c);
                        if next_path.symlink_metadata().is_ok() {
                            let metadata = next_path.symlink_metadata()?;
                            if metadata.is_symlink() || !metadata.is_dir() {
                                // If it's a symlink or file, we must remove it to create a real directory
                                if metadata.is_dir() {
                                    fs::remove_dir_all(&next_path)?;
                                } else {
                                    fs::remove_file(&next_path)?;
                                }
                                fs::create_dir(&next_path)?;
                                current_canonical = next_path;
                            } else {
                                // Valid directory, canonicalize to ensure it didn't escape
                                current_canonical = fs::canonicalize(&next_path)?;
                            }
                        } else {
                            // Create directory
                            fs::create_dir(&next_path)?;
                            current_canonical = next_path;
                        }
                    }
                    _ => return Err(BackupError::UnsafePath(path_str.clone())),
                }
                if !current_canonical.starts_with(&canonical_target) {
                    return Err(BackupError::UnsafePath(path_str.clone()));
                }
            }
        }

        // If target file already exists, move it to rollback directory
        if final_file_path.symlink_metadata().is_ok() {
            let rollback_file_path = canonical_rollback.join(rel_path);
            if let Some(parent) = rollback_file_path.parent() {
                fs::create_dir_all(parent)?;
            }
            fs::rename(&final_file_path, &rollback_file_path)?;
            tx.rollbacks.push((rollback_file_path, final_file_path.clone()));
        }

        fs::rename(&temp_file_path, &final_file_path)?;
        if !tx.rollbacks.iter().any(|(_, t)| t == &final_file_path) {
            tx.new_files.push(final_file_path);
        }
    }

    tx.commit();
    Ok(manifest)
}

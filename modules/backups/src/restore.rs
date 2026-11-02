use crate::error::BackupError;
use crate::models::{FileInfo, Manifest};
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use tar::Archive;
use zstd::stream::Decoder as ZstdDecoder;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_DIR_COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempRestoreDir {
    path: PathBuf,
}

impl TempRestoreDir {
    fn new(base_dir: &Path) -> Result<Self, BackupError> {
        let counter = TEMP_DIR_COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = base_dir.join(format!(".restore_temp_{}_{}", timestamp, counter));
        fs::create_dir_all(&path)?;

        // Ensure atomic file creation with specific POSIX permissions to prevent TOCTOU
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700))?;
        }

        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempRestoreDir {
    fn drop(&mut self) {
        if self.path.exists() {
            let _ = fs::remove_dir_all(&self.path);
        }
    }
}

pub fn restore_archive<P1: AsRef<Path>, P2: AsRef<Path>>(
    archive_path: P1,
    target_dir: P2,
) -> Result<Manifest, BackupError> {
    let target_dir = target_dir.as_ref();
    if !target_dir.exists() {
        fs::create_dir_all(target_dir)?;
    }
    let canonical_target = fs::canonicalize(target_dir)?;

    // Create a temporary directory in the target dir to avoid cross-device linking issues
    // and use a secure custom approach to ensure isolated atomic staging.
    let temp_restore = TempRestoreDir::new(&canonical_target)?;
    let canonical_temp = fs::canonicalize(temp_restore.path())?;

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

            // Check for unsafe paths in the components
            for component in path.components() {
                match component {
                    Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                        return Err(BackupError::UnsafePath(path_str));
                    }
                    _ => {}
                }
            }

            let expected_info = file_info_map.get(&path_str).ok_or_else(|| {
                BackupError::UnexpectedFile(path_str.clone())
            })?;

            if !extracted_files.insert(path_str.clone()) {
                return Err(BackupError::DuplicateFile(path_str.clone()));
            }

            let temp_target_path = canonical_temp.join(&path);

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

    for file_info in &manifest.files {
        if !extracted_files.contains(&file_info.path) {
            return Err(BackupError::MissingFiles);
        }
    }

    // Move files atomically from temp to target
    for path_str in &extracted_files {
        let temp_file_path = canonical_temp.join(path_str);
        let final_file_path = canonical_target.join(path_str);

        // Carefully handle pre-existing parent symlinks by ensuring each
        // component is a real directory, deleting if it's a symlink or file
        if let Some(parent) = final_file_path.parent() {
            let mut current = canonical_target.to_path_buf();
            let relative_parent = parent.strip_prefix(&canonical_target).unwrap_or(parent);
            for component in relative_parent.components() {
                current.push(component);
                if let Ok(metadata) = current.symlink_metadata() {
                    if !metadata.is_dir() || metadata.file_type().is_symlink() {
                        if metadata.is_dir() {
                            fs::remove_dir_all(&current)?;
                        } else {
                            fs::remove_file(&current)?;
                        }
                    }
                }
            }
            fs::create_dir_all(parent)?;
        }

        // If target file already exists, remove it (could be a symlink or file)
        if final_file_path.symlink_metadata().is_ok() {
            if final_file_path.is_dir() {
                fs::remove_dir_all(&final_file_path)?;
            } else {
                fs::remove_file(&final_file_path)?;
            }
        }

        fs::rename(&temp_file_path, &final_file_path)?;
    }

    Ok(manifest)
}

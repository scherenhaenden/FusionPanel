use crate::error::BackupError;
use crate::models::{FileInfo, Manifest};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Component, Path};
use tar::Archive;

pub fn restore_archive<P1: AsRef<Path>, P2: AsRef<Path>>(
    archive_path: P1,
    target_dir: P2,
) -> Result<Manifest, BackupError> {
    let target_dir = target_dir.as_ref();
    if !target_dir.exists() {
        fs::create_dir_all(target_dir)?;
    }

    // Staging directory
    let staging_dir = target_dir.join(".staging");
    if staging_dir.exists() {
        fs::remove_dir_all(&staging_dir)?;
    }
    fs::create_dir_all(&staging_dir)?;

    let file = File::open(archive_path)?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);

    let mut manifest: Option<Manifest> = None;
    let mut file_info_map: HashMap<String, FileInfo> = HashMap::new();
    let mut extracted_files: std::collections::HashSet<String> = std::collections::HashSet::new();

    let result: Result<Manifest, BackupError> = (|| {
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
                for file_info in &parsed_manifest.files {
                    file_info_map.insert(file_info.path.clone(), file_info.clone());
                }
                manifest = Some(parsed_manifest);
            } else {
                // Must have seen manifest first (or we error out for now)
                if manifest.is_none() {
                    return Err(BackupError::ManifestNotFound);
                }

                // Check for unsafe paths
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

                let staging_path = staging_dir.join(&path);
                if let Some(parent) = staging_path.parent() {
                    fs::create_dir_all(parent)?;
                }
                let mut dest_file = File::create(&staging_path)?;

                // Stream and hash simultaneously
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

        if extracted_files.len() != file_info_map.len() {
            return Err(BackupError::MissingFiles);
        }

        Ok(manifest)
    })();

    match result {
        Ok(manifest) => {
            // Commit: move files from staging to target_dir
            for path_str in extracted_files {
                let staging_path = staging_dir.join(&path_str);
                let target_path = target_dir.join(&path_str);

                // Carefully handle pre-existing parent symlinks by ensuring each
                // component is a real directory, deleting if it's a symlink or file
                if let Some(parent) = target_path.parent() {
                    let mut current = target_dir.to_path_buf();
                    let relative_parent = parent.strip_prefix(target_dir).unwrap_or(parent);
                    for component in relative_parent.components() {
                        current.push(component);
                        if let Ok(metadata) = current.symlink_metadata() {
                            if !metadata.is_dir() || metadata.file_type().is_symlink() {
                                if metadata.is_dir() { // Just in case it's a symlink to a dir
                                    fs::remove_dir_all(&current)?;
                                } else {
                                    fs::remove_file(&current)?;
                                }
                            }
                        }
                    }
                    fs::create_dir_all(parent)?;
                }

                // If it already exists, carefully remove it (could be a symlink)
                if target_path.symlink_metadata().is_ok() {
                    if target_path.is_dir() {
                        fs::remove_dir_all(&target_path)?;
                    } else {
                        fs::remove_file(&target_path)?;
                    }
                }

                fs::rename(staging_path, target_path)?;
            }

            // Clean up empty staging directory
            let _ = fs::remove_dir_all(&staging_dir);
            Ok(manifest)
        }
        Err(e) => {
            // Rollback: clean up staging directory
            let _ = fs::remove_dir_all(&staging_dir);
            Err(e)
        }
    }
}

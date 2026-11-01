use crate::error::BackupError;
use crate::models::{FileInfo, Manifest};
use flate2::read::GzDecoder;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Read, Write};
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

    let file = File::open(archive_path)?;
    let tar = GzDecoder::new(file);
    let mut archive = Archive::new(tar);

    let mut manifest: Option<Manifest> = None;
    let mut file_info_map: HashMap<String, FileInfo> = HashMap::new();

    let entries = archive.entries()?;
    for entry in entries {
        let mut entry = entry?;
        let path = entry.path()?.to_path_buf();
        let path_str = path.to_string_lossy().into_owned();

        if path_str == "manifest.json" {
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
                BackupError::Io(io::Error::new(io::ErrorKind::NotFound, "File in archive not in manifest"))
            })?;

            let target_path = target_dir.join(&path);
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)?;
            }
            let mut dest_file = File::create(&target_path)?;

            // Stream and hash simultaneously
            let mut hasher = Sha256::new();
            let mut buffer = [0; 8192];
            loop {
                let count = entry.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
                dest_file.write_all(&buffer[..count])?;
            }

            let hash = hex::encode(hasher.finalize());

            if hash != expected_info.checksum_sha256 {
                // Remove invalid file
                let _ = fs::remove_file(&target_path);
                return Err(BackupError::ChecksumMismatch {
                    path: path_str.clone(),
                    expected: expected_info.checksum_sha256.clone(),
                    actual: hash,
                });
            }
        }
    }

    manifest.ok_or(BackupError::ManifestNotFound)
}

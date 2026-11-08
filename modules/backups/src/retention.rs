use crate::error::BackupError;
use crate::models::RetentionPolicy;
use flate2::read::GzDecoder;
use std::fs;
use std::fs::File;
use std::path::{Path, PathBuf};
use tar::Archive;
use zstd::stream::Decoder as ZstdDecoder;

pub fn apply_retention_policy<P: AsRef<Path>>(
    backup_dir: P,
    policy: &RetentionPolicy,
) -> Result<Vec<PathBuf>, BackupError> {
    let mut backups: Vec<(PathBuf, chrono::DateTime<chrono::Utc>)> = Vec::new();

    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            // Prove backup ownership: read magic bytes, then parse and validate manifest.json
            let is_valid_backup = (|| -> Result<bool, BackupError> {
                let mut file = File::open(&path)?;
                let mut magic = [0u8; 4];
                use std::io::Read;
                if file.read(&mut magic)? < 4 {
                    return Ok(false);
                }

                // Rewind file to start
                use std::io::Seek;
                use std::io::SeekFrom;
                file.seek(SeekFrom::Start(0))?;

                let is_zst = magic == [0x28, 0xB5, 0x2F, 0xFD];
                let is_gz = magic.starts_with(&[0x1F, 0x8B]);

                if !is_zst && !is_gz {
                    return Ok(false);
                }

                let manifest_data = if is_zst {
                    let tar = ZstdDecoder::new(file)?;
                    let mut archive = Archive::new(tar);
                    let mut entries = archive.entries()?;
                    let mut entry = match entries.next() {
                        Some(Ok(e)) => e,
                        _ => return Ok(false),
                    };
                    if entry.path()?.to_string_lossy() != "manifest.json" {
                        return Ok(false);
                    }
                    let mut content = Vec::new();
                    entry.read_to_end(&mut content)?;
                    content
                } else {
                    let tar = GzDecoder::new(file);
                    let mut archive = Archive::new(tar);
                    let mut entries = archive.entries()?;
                    let mut entry = match entries.next() {
                        Some(Ok(e)) => e,
                        _ => return Ok(false),
                    };
                    if entry.path()?.to_string_lossy() != "manifest.json" {
                        return Ok(false);
                    }
                    let mut content = Vec::new();
                    entry.read_to_end(&mut content)?;
                    content
                };

                let manifest: crate::models::Manifest = serde_json::from_slice(&manifest_data)?;
                manifest.validate()?;
                Ok(true)
            })()
            .unwrap_or(false);

            if is_valid_backup {
                let metadata = fs::metadata(&path)?;
                if let Ok(modified) = metadata.modified() {
                    backups.push((path, modified.into()));
                }
            }
        }
    }

    // Sort by modified time descending (newest first)
    backups.sort_by(|a, b| b.1.cmp(&a.1));

    let mut to_delete = Vec::new();

    if let Some(max_backups) = policy.max_backups {
        if backups.len() > max_backups {
            for (path, _) in backups.iter().skip(max_backups) {
                to_delete.push(path.clone());
            }
        }
    }

    if let Some(max_days) = policy.max_days {
        let cutoff = chrono::Utc::now() - chrono::Duration::days(max_days as i64);
        for (path, time) in backups.iter() {
            if *time < cutoff && !to_delete.contains(path) {
                to_delete.push(path.clone());
            }
        }
    }

    for path in &to_delete {
        fs::remove_file(path)?;
    }

    Ok(to_delete)
}

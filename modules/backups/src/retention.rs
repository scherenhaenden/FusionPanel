use crate::error::BackupError;
use crate::models::RetentionPolicy;
use std::fs;
use std::path::{Path, PathBuf};
use flate2::read::GzDecoder;
use std::fs::File;
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
        let ext = path.extension().and_then(|s| s.to_str());
        if path.is_file() && (ext == Some("zst") || ext == Some("gz")) {
            // Prove backup ownership: check if it's a valid backup archive with manifest.json
            let is_valid_backup = (|| -> Result<bool, std::io::Error> {
                let file = File::open(&path)?;
                if ext == Some("zst") {
                    let tar = ZstdDecoder::new(file)?;
                    let mut archive = Archive::new(tar);
                    if let Some(Ok(entry)) = archive.entries()?.next() {
                        let entry_path = entry.path()?.to_path_buf();
                        let path_str = entry_path.to_string_lossy().into_owned();
                        return Ok(path_str == "manifest.json");
                    }
                } else {
                    let tar = GzDecoder::new(file);
                    let mut archive = Archive::new(tar);
                    if let Some(Ok(entry)) = archive.entries()?.next() {
                        let entry_path = entry.path()?.to_path_buf();
                        let path_str = entry_path.to_string_lossy().into_owned();
                        return Ok(path_str == "manifest.json");
                    }
                }
                Ok(false)
            })().unwrap_or(false);

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

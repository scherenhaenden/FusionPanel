use crate::error::BackupError;
use crate::models::RetentionPolicy;
use std::fs;
use std::path::{Path, PathBuf};

pub fn apply_retention_policy<P: AsRef<Path>>(
    backup_dir: P,
    policy: &RetentionPolicy,
) -> Result<Vec<PathBuf>, BackupError> {
    let mut backups: Vec<(PathBuf, chrono::DateTime<chrono::Utc>)> = Vec::new();

    for entry in fs::read_dir(backup_dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("gz") {
            let metadata = fs::metadata(&path)?;
            if let Ok(modified) = metadata.modified() {
                backups.push((path, modified.into()));
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

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpdateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Missing target binary path")]
    MissingTargetPath,
}

pub struct Updater {
    target_path: PathBuf,
}

impl Updater {
    pub fn new(target_path: impl Into<PathBuf>) -> Self {
        Self {
            target_path: target_path.into(),
        }
    }

    /// Performs an in-place update by atomically swapping the binary and 
    /// providing crash resilience via a `.recovery-marker` file.
    pub fn update(&self, new_binary_path: &Path) -> Result<(), UpdateError> {
        let parent = self
            .target_path
            .parent()
            .ok_or(UpdateError::MissingTargetPath)?;

        let backup_path = parent.join(format!(
            "{}.backup",
            self.target_path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        ));
        
        let marker_path = parent.join(".recovery-marker");

        // 1. Ensure new binary has correct permissions (0o755) before atomic swap
        let mut perms = fs::metadata(new_binary_path)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(new_binary_path, perms)?;

        // 2. Write durable recovery marker before renaming backup artifacts
        fs::write(&marker_path, b"update_in_progress")?;

        // 3. Backup current active binary if it exists
        if self.target_path.exists() {
            fs::rename(&self.target_path, &backup_path)?;
        }

        // 4. Atomic swap (rename new binary to target path)
        if let Err(e) = fs::rename(new_binary_path, &self.target_path) {
            // Rollback on failure
            if backup_path.exists() {
                let _ = fs::rename(&backup_path, &self.target_path);
            }
            // Retain marker so system knows crash/rollback happened mid-update
            return Err(UpdateError::Io(e));
        }

        // 5. Cleanup marker and backup on success
        let _ = fs::remove_file(&marker_path);
        let _ = fs::remove_file(&backup_path);

        Ok(())
    }

    pub fn check_recovery(&self) -> Result<bool, std::io::Error> {
        let parent = match self.target_path.parent() {
            Some(p) => p,
            None => return Ok(false),
        };
        let marker_path = parent.join(".recovery-marker");
        Ok(marker_path.exists())
    }
}

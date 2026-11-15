#![forbid(unsafe_code)]

use std::path::{Path, PathBuf};
use std::process::Command;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum EtckeeperError {
    #[error("Git command failed: {0}")]
    GitCommandFailed(String),
    #[error("Uncommitted drift detected: {0}")]
    UncommittedDriftDetected(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Invalid path")]
    InvalidPath,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriftPolicy {
    Strict,
    AutoCommit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitStatus {
    pub modified: Vec<String>,
    pub added: Vec<String>,
    pub deleted: Vec<String>,
    pub untracked: Vec<String>,
}

impl GitStatus {
    pub fn has_drift(&self) -> bool {
        !self.modified.is_empty()
            || !self.added.is_empty()
            || !self.deleted.is_empty()
            || !self.untracked.is_empty()
    }
}

pub struct EtckeeperGuard {
    etc_path: PathBuf,
    policy: DriftPolicy,
}

impl EtckeeperGuard {
    pub fn new<P: AsRef<Path>>(etc_path: P, policy: DriftPolicy) -> Self {
        Self {
            etc_path: etc_path.as_ref().to_path_buf(),
            policy,
        }
    }

    pub fn get_status(&self) -> Result<GitStatus, EtckeeperError> {
        let output = Command::new("git")
            .current_dir(&self.etc_path)
            .args(&["status", "--porcelain"])
            .output()
            .map_err(|e| EtckeeperError::IoError(e))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();
            return Err(EtckeeperError::GitCommandFailed(stderr));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut status = GitStatus {
            modified: Vec::new(),
            added: Vec::new(),
            deleted: Vec::new(),
            untracked: Vec::new(),
        };

        for line in stdout.lines() {
            if line.len() < 4 {
                continue;
            }
            let (prefix, file) = line.split_at(3);
            let file = file.trim().to_string();
            let prefix = prefix.trim_end();

            match prefix {
                "??" => status.untracked.push(file),
                "M" | " M" | "MM" | "AM" | "RM" | "CM" => status.modified.push(file),
                "A" | " A" => status.added.push(file),
                "D" | " D" | "AD" | "MD" => status.deleted.push(file),
                "R" | " R" => {
                    // Usually "R  old -> new", we just care it's modified/added
                    // Let's just put it in modified for drift purposes
                    status.modified.push(file);
                }
                _ => {
                    // Treat any other prefix as modified if not empty
                    if !prefix.is_empty() {
                        status.modified.push(file);
                    }
                }
            }
        }

        Ok(status)
    }

    fn commit_all(&self, message: &str) -> Result<(), EtckeeperError> {
        // Add all changes including untracked
        let add_output = Command::new("git")
            .current_dir(&self.etc_path)
            .args(&["add", "-A"])
            .output()?;

        if !add_output.status.success() {
            let stderr = String::from_utf8_lossy(&add_output.stderr).to_string();
            return Err(EtckeeperError::GitCommandFailed(format!("git add failed: {}", stderr)));
        }

        let commit_output = Command::new("git")
            .current_dir(&self.etc_path)
            .args(&["commit", "-m", message])
            .output()?;

        if !commit_output.status.success() {
            let stderr = String::from_utf8_lossy(&commit_output.stderr).to_string();
            // git commit fails if there is nothing to commit, which is fine
            if !stderr.contains("nothing to commit") && !String::from_utf8_lossy(&commit_output.stdout).contains("nothing to commit") {
                return Err(EtckeeperError::GitCommandFailed(format!("git commit failed: {}", stderr)));
            }
        }

        Ok(())
    }

    pub fn prepare_for_update(&self, action_description: &str) -> Result<(), EtckeeperError> {
        let status = self.get_status()?;

        if status.has_drift() {
            match self.policy {
                DriftPolicy::Strict => {
                    return Err(EtckeeperError::UncommittedDriftDetected(
                        "Found uncommitted changes in /etc".to_string(),
                    ));
                }
                DriftPolicy::AutoCommit => {
                    self.commit_all("[FusionPanel] Auto-commit: resolving drift before updates")?;
                }
            }
        }

        // We can just commit anything that might be there, though it should be clean now
        let msg = format!("[FusionPanel] Pre-update: {}", action_description);
        self.commit_all(&msg)?;

        Ok(())
    }

    pub fn post_update_commit(&self, action_description: &str) -> Result<(), EtckeeperError> {
        let status = self.get_status()?;
        if status.has_drift() {
            let msg = format!("[FusionPanel] Post-update: {}", action_description);
            self.commit_all(&msg)?;
        }
        Ok(())
    }
}

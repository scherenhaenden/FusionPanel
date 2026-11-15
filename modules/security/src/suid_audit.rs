#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum SuidAuditError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RiskLevel {
    Critical,
    Suspicious,
    Standard,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SuidFinding {
    pub path: PathBuf,
    pub risk_level: RiskLevel,
    pub is_suid: bool,
    pub is_sgid: bool,
}

pub struct SuidScanner {
    gtfobins: HashSet<String>,
    standard_bins: HashSet<String>,
}

impl Default for SuidScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl SuidScanner {
    pub fn new() -> Self {
        let gtfobins: HashSet<String> = vec![
            "find", "vim", "cp", "bash", "awk", "perl", "python", "nmap",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        let standard_bins: HashSet<String> = vec![
            "passwd", "sudo", "pkexec", "su", "chsh", "mount", "umount", "ping",
            "fusermount", "fusermount3", "chfn", "newgrp",
        ]
        .into_iter()
        .map(String::from)
        .collect();

        Self {
            gtfobins,
            standard_bins,
        }
    }

    pub fn scan_path<P: AsRef<Path>>(&self, path: P) -> Result<Vec<SuidFinding>, SuidAuditError> {
        let mut findings = Vec::new();
        self.scan_recursive(path.as_ref(), &mut findings)?;
        Ok(findings)
    }

    fn scan_recursive(
        &self,
        path: &Path,
        findings: &mut Vec<SuidFinding>,
    ) -> Result<(), SuidAuditError> {
        if path.is_dir() {
            for entry in fs::read_dir(path)? {
                let entry = entry?;
                let file_type = entry.file_type()?;
                if file_type.is_dir() {
                    self.scan_recursive(&entry.path(), findings)?;
                } else if file_type.is_file() {
                    if let Some(finding) = self.evaluate_file(&entry.path())? {
                        findings.push(finding);
                    }
                }
            }
        } else if path.is_file() {
            if let Some(finding) = self.evaluate_file(path)? {
                findings.push(finding);
            }
        }
        Ok(())
    }

    fn evaluate_file(&self, path: &Path) -> Result<Option<SuidFinding>, SuidAuditError> {
        let metadata = fs::symlink_metadata(path)?;
        if !metadata.is_file() {
            return Ok(None);
        }

        let mode = metadata.permissions().mode();
        let is_suid = (mode & 0o4000) != 0;
        let is_sgid = (mode & 0o2000) != 0;

        if !is_suid && !is_sgid {
            return Ok(None);
        }

        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        let in_standard_dir = path.starts_with("/usr/bin") || path.starts_with("/bin");

        let risk_level = if self.gtfobins.contains(&file_name) {
            RiskLevel::Critical
        } else if self.standard_bins.contains(&file_name) || in_standard_dir {
            // If it's a known standard bin, or it's inside a standard directory but not a known GTFOBin
            // However, to strictly meet "Suspicious (non-standard SUID binary outside /usr/bin or /bin)",
            // binaries inside /usr/bin or /bin that are not GTFOBins are treated as Standard.
            // Wait, what if it's just non-standard inside /usr/bin? The instructions imply Suspicious is only outside.
            // Let's do exactly what it says.
            RiskLevel::Standard
        } else {
            // Non-standard and outside /usr/bin or /bin
            RiskLevel::Suspicious
        };

        Ok(Some(SuidFinding {
            path: path.to_path_buf(),
            risk_level,
            is_suid,
            is_sgid,
        }))
    }

    pub fn compare_baseline(
        &self,
        baseline: &[SuidFinding],
        current: &[SuidFinding],
    ) -> Vec<SuidFinding> {
        let baseline_paths: HashSet<_> = baseline.iter().map(|f| f.path.clone()).collect();
        current
            .iter()
            .filter(|f| !baseline_paths.contains(&f.path))
            .cloned()
            .collect()
    }
}

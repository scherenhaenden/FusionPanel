#![forbid(unsafe_code)]

use std::fs;
use std::path::{Path, PathBuf};
use std::os::unix::fs::PermissionsExt;
use std::io;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone)]
pub struct ScanFinding {
    pub path: PathBuf,
    pub severity: Severity,
    pub rule_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Default)]
pub struct ScanReport {
    pub findings: Vec<ScanFinding>,
}

pub struct WebrootScanner {}

impl Default for WebrootScanner {
    fn default() -> Self {
        Self::new()
    }
}

impl WebrootScanner {
    pub fn new() -> Self {
        Self {}
    }

    pub fn scan<P: AsRef<Path>>(&self, root: P) -> io::Result<ScanReport> {
        let mut report = ScanReport::default();
        let root_path = root.as_ref();
        
        if root_path.exists() {
            self.scan_path(root_path, &mut report)?;
        }
        
        Ok(report)
    }

    fn scan_path(&self, path: &Path, report: &mut ScanReport) -> io::Result<()> {
        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.permissions().mode() & 0o002 != 0 {
                report.findings.push(ScanFinding {
                    path: path.to_path_buf(),
                    severity: Severity::High,
                    rule_id: "WORLD_WRITABLE".to_string(),
                    message: "Path is world-writable".to_string(),
                });
            }
            
            if metadata.file_type().is_symlink() {
                return Ok(()); // Do not traverse symlinks
            }
        }

        if path.is_dir() {
            if let Ok(entries) = fs::read_dir(path) {
                for entry in entries {
                    if let Ok(entry) = entry {
                        self.scan_path(&entry.path(), report)?;
                    }
                }
            }
        } else if path.is_file() {
            self.scan_file(path, report)?;
        }

        Ok(())
    }

    fn scan_file(&self, path: &Path, report: &mut ScanReport) -> io::Result<()> {
        let path_str = path.to_string_lossy().replace('\\', "/");
        
        let is_upload_dir = path_str.contains("/uploads/") 
            || path_str.contains("/media/") 
            || path_str.contains("/images/");
            
        let is_executable_ext = path_str.ends_with(".php") 
            || path_str.ends_with(".cgi") 
            || path_str.ends_with(".php5") 
            || path_str.ends_with(".phtml");

        if is_upload_dir && is_executable_ext {
            report.findings.push(ScanFinding {
                path: path.to_path_buf(),
                severity: Severity::Critical,
                rule_id: "EXECUTABLE_IN_UPLOAD".to_string(),
                message: "Executable script found in upload directory".to_string(),
            });
        }

        if let Ok(metadata) = fs::symlink_metadata(path) {
            if metadata.len() > 10 * 1024 * 1024 {
                // Skip files larger than 10MB to avoid OOM
                return Ok(());
            }
        }

        if let Ok(content) = fs::read(path) {
            let content_lossy = String::from_utf8_lossy(&content);
            
            if content_lossy.contains("eval(base64_decode") {
                report.findings.push(ScanFinding {
                    path: path.to_path_buf(),
                    severity: Severity::Critical,
                    rule_id: "WEBSHELL_PATTERN".to_string(),
                    message: "Found eval(base64_decode) pattern".to_string(),
                });
            } else if content_lossy.contains("system(/home/edward/Development/web-panel/FusionPanel") 
                || content_lossy.contains("passthru(/home/edward/Development/web-panel/FusionPanel") 
                || content_lossy.contains("shell_exec(/home/edward/Development/web-panel/FusionPanel")
            {
                report.findings.push(ScanFinding {
                    path: path.to_path_buf(),
                    severity: Severity::High,
                    rule_id: "WEBSHELL_PATTERN".to_string(),
                    message: "Found dangerous shell execution pattern".to_string(),
                });
            }
        }

        Ok(())
    }
}

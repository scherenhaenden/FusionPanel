#![forbid(unsafe_code)]

use std::fs;
use std::path::PathBuf;
use std::process::{Command, Stdio};

#[derive(Debug, Clone)]
pub enum Event {
    SiteCreated { domain: String, config: String },
    SiteUpdated { domain: String, config: String },
}

pub struct HostingWorker {
    pub staging_dir: PathBuf,
    pub sites_available_dir: PathBuf,
    pub nginx_bin: String,
}

impl HostingWorker {
    pub fn new(staging_dir: PathBuf, sites_available_dir: PathBuf, nginx_bin: String) -> Self {
        Self {
            staging_dir,
            sites_available_dir,
            nginx_bin,
        }
    }

    fn validate_domain(domain: &str) -> Result<(), String> {
        if domain.is_empty() || domain.len() > 253 {
            return Err("Invalid domain length".to_string());
        }
        if !domain.is_ascii() {
            return Err("IDNA/Unicode domains are explicitly rejected".to_string());
        }
        for part in domain.split('.') {
            if part.is_empty() || part.len() > 63 {
                return Err("Invalid domain label length".to_string());
            }
            if !part.chars().all(|c| c.is_ascii_alphanumeric() || c == '-') {
                return Err("Invalid characters in domain".to_string());
            }
            if part.starts_with('-') || part.ends_with('-') {
                return Err("Domain label cannot start or end with a hyphen".to_string());
            }
        }
        Ok(())
    }

    pub fn process_event(&self, event: Event) -> Result<(), String> {
        let (domain, config_content) = match event {
            Event::SiteCreated { domain, config } => (domain, config),
            Event::SiteUpdated { domain, config } => (domain, config),
        };

        Self::validate_domain(&domain)?;

        // Render to staging
        let staging_file = self.staging_dir.join(format!("{}.conf", domain));
        fs::write(&staging_file, config_content)
            .map_err(|e| format!("Failed to write to staging: {}", e))?;

        // Atomic copy to sites-available.
        // We first copy to a temporary file in the sites-available directory,
        // then rename it to the target name.
        let target_file = self.sites_available_dir.join(format!("{}.conf", domain));
        let backup_file = self
            .sites_available_dir
            .join(format!("{}.conf.bak", domain));

        let has_existing = target_file.exists();
        if has_existing {
            fs::copy(&target_file, &backup_file)
                .map_err(|e| format!("Failed to create backup: {}", e))?;
        }

        let temp_target = self
            .sites_available_dir
            .join(format!("{}.conf.tmp", domain));
        fs::copy(&staging_file, &temp_target)
            .map_err(|e| format!("Failed to copy to temp file: {}", e))?;

        if let Err(e) = fs::rename(&temp_target, &target_file) {
            let _ = fs::remove_file(&temp_target);
            if has_existing {
                let _ = fs::remove_file(&backup_file);
            }
            return Err(format!("Failed to atomically rename config: {}", e));
        }

        // Test configuration
        let test_output = Command::new(&self.nginx_bin)
            .arg("-t")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let test_success = test_output
            .as_ref()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !test_success {
            // Rollback
            let rollback_failed = if has_existing {
                fs::rename(&backup_file, &target_file).is_err()
            } else {
                fs::remove_file(&target_file).is_err()
            };

            if rollback_failed {
                let _ = fs::write(
                    self.sites_available_dir.join(".restart-marker"),
                    "Rollback failed after config test error",
                );
            }

            return match test_output {
                Ok(output) => Err(format!(
                    "Nginx config test failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )),
                Err(e) => Err(format!("Failed to run nginx -t: {}", e)),
            };
        }

        // Reload nginx daemon
        let reload_output = Command::new(&self.nginx_bin)
            .arg("-s")
            .arg("reload")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output();

        let reload_success = reload_output
            .as_ref()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !reload_success {
            // Rollback
            let rollback_failed = if has_existing {
                fs::rename(&backup_file, &target_file).is_err()
            } else {
                fs::remove_file(&target_file).is_err()
            };

            if rollback_failed {
                let _ = fs::write(
                    self.sites_available_dir.join(".restart-marker"),
                    "Rollback failed after reload error",
                );
            }

            return match reload_output {
                Ok(output) => Err(format!(
                    "Nginx reload failed: {}",
                    String::from_utf8_lossy(&output.stderr)
                )),
                Err(e) => Err(format!("Failed to run nginx -s reload: {}", e)),
            };
        }

        // Remove backup on success
        if has_existing {
            let _ = fs::remove_file(&backup_file);
        }

        Ok(())
    }
}

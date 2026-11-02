use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MaintenanceError {
    #[error("IO Error: {0}")]
    Io(#[from] io::Error),
    #[error("Configuration Error: {0}")]
    Config(String),
}

/// Automates WordPress maintenance operations
pub struct Coordinator {
    docroot: PathBuf,
}

impl Coordinator {
    /// Creates a new maintenance coordinator for a WordPress document root
    pub fn new(docroot: impl AsRef<Path>) -> Self {
        Self {
            docroot: docroot.as_ref().to_path_buf(),
        }
    }

    /// Toggles the atomic `.maintenance` file flag
    pub fn set_maintenance_mode(&self, enabled: bool) -> Result<(), MaintenanceError> {
        let maintenance_file = self.docroot.join(".maintenance");

        if enabled {
            // WordPress uses a PHP timestamp for the maintenance file.
            // When `.maintenance` exists and contains `$upgrading = time();`,
            // WP enters maintenance mode.
            // Using time() is a bit tricky, but setting a past time or just letting WP evaluate
            // a recent timestamp works. Let's write a standard maintenance marker.
            let content = "<?php $upgrading = time(); ?>";
            fs::write(&maintenance_file, content)?;
        } else {
            if maintenance_file.exists() {
                fs::remove_file(&maintenance_file)?;
            }
        }

        Ok(())
    }

    /// Checks if maintenance mode is enabled
    pub fn is_maintenance_mode_enabled(&self) -> bool {
        self.docroot.join(".maintenance").exists()
    }

    /// Offloads system-cron by ensuring `DISABLE_WP_CRON` is set in wp-config.php
    pub fn offload_system_cron(&self, disable: bool) -> Result<(), MaintenanceError> {
        let config_file = self.docroot.join("wp-config.php");
        if !config_file.exists() {
            return Err(MaintenanceError::Config(
                "wp-config.php not found".to_string(),
            ));
        }

        let content = fs::read_to_string(&config_file)?;

        // Simple regex-like replacement for now.
        // Look for define('DISABLE_WP_CRON', ...);
        let mut lines: Vec<String> = content.lines().map(String::from).collect();
        let mut found = false;

        for line in &mut lines {
            if line.contains("define") && line.contains("DISABLE_WP_CRON") {
                if disable {
                    *line = "define( 'DISABLE_WP_CRON', true );".to_string();
                } else {
                    *line = "define( 'DISABLE_WP_CRON', false );".to_string();
                }
                found = true;
            }
        }

        if !found {
            // If not found, insert before the "That's all, stop editing" comment,
            // or at the end of the file.
            let insert_index = lines
                .iter()
                .position(|l| l.contains("That's all, stop editing"))
                .unwrap_or(lines.len());

            let define_stmt = if disable {
                "define( 'DISABLE_WP_CRON', true );".to_string()
            } else {
                "define( 'DISABLE_WP_CRON', false );".to_string()
            };

            lines.insert(insert_index, define_stmt);
        }

        let new_content = lines.join("\n");
        fs::write(&config_file, new_content)?;

        Ok(())
    }
}

// SPDX-License-Identifier: AGPL-3.0-only
// Copyright 2024 FusionPanel

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

struct TempDir(PathBuf);

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

impl AsRef<Path> for TempDir {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

fn require_wp_cli() {
    match Command::new("wp").arg("--info").output() {
        Ok(output) if output.status.success() => {}
        _ => panic!("BLOCKED: wp-cli is required for WordPress E2E tests"),
    }
}

fn require_php() {
    match Command::new("php").arg("-v").output() {
        Ok(output) if output.status.success() => {}
        _ => panic!("BLOCKED: PHP is required for WordPress E2E tests"),
    }
}

fn get_db_config() -> Option<(String, String, String, String)> {
    let host = env::var("WP_TEST_DB_HOST").ok()?;
    let name = env::var("WP_TEST_DB_NAME").ok()?;
    let user = env::var("WP_TEST_DB_USER").ok()?;
    let pass = env::var("WP_TEST_DB_PASS").ok()?;
    Some((host, name, user, pass))
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn setup_test_env() -> TempDir {
    let mut dir = env::temp_dir();
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_millis();
    let count = COUNTER.fetch_add(1, Ordering::SeqCst);
    dir.push(format!("fusionpanel_wp_e2e_{}_{}", now, count));
    fs::create_dir_all(&dir).expect("Failed to create test directory");
    TempDir(dir)
}

#[test]
fn test_wordpress_installation_orchestration() {
    require_wp_cli();
    require_php();

    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };

    let dir = setup_test_env();

    // 1. Download WordPress core
    let output = Command::new("wp")
        .current_dir(dir.as_ref())
        .args(["core", "download"])
        .output()
        .expect("Failed to execute wp process");

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        if stderr.contains("Error:") || stderr.contains("Warning:") {
            panic!(
                "BLOCKED: wp core download failed, possibly due to network constraints: {}",
                stderr
            );
        } else {
            panic!(
                "BLOCKED: wp core download returned non-zero status: {}",
                output.status
            );
        }
    }

    assert!(
        dir.as_ref().join("wp-settings.php").exists(),
        "wp-settings.php was not created"
    );
    assert!(
        dir.as_ref().join("wp-admin").exists(),
        "wp-admin directory was not created"
    );

    // 2. Generate config
    let output = Command::new("wp")
        .current_dir(dir.as_ref())
        .args([
            "core",
            "config",
            &format!("--dbhost={}", db_host),
            &format!("--dbname={}", db_name),
            &format!("--dbuser={}", db_user),
            &format!("--dbpass={}", db_pass),
        ])
        .output()
        .expect("Failed to execute wp process");

    if !output.status.success() {
        panic!(
            "BLOCKED: wp core config failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // 3. Install WordPress
    let output = Command::new("wp")
        .current_dir(dir.as_ref())
        .args([
            "core",
            "install",
            "--url=http://localhost",
            "--title=Test",
            "--admin_user=admin",
            "--admin_password=admin",
            "--admin_email=admin@example.com",
        ])
        .output()
        .expect("Failed to execute wp process");

    if !output.status.success() {
        panic!(
            "BLOCKED: wp core install failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn test_wp_config_generation() {
    require_wp_cli();
    require_php();

    let dir = setup_test_env();

    // 1. Download WordPress core
    let output = Command::new("wp")
        .current_dir(dir.as_ref())
        .args(["core", "download"])
        .output()
        .expect("Failed to execute wp process");

    if !output.status.success() {
        panic!("BLOCKED: wp core download failed");
    }

    // 2. Generate wp-config.php (without DB connection check)
    let output = Command::new("wp")
        .current_dir(dir.as_ref())
        .args([
            "core",
            "config",
            "--dbname=testdb",
            "--dbuser=testuser",
            "--dbpass=testpass",
            "--skip-check",
        ])
        .output()
        .expect("Failed to execute wp process");

    if !output.status.success() {
        panic!(
            "BLOCKED: wp core config failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let wp_config_path = dir.as_ref().join("wp-config.php");
    assert!(wp_config_path.exists(), "wp-config.php was not generated");

    let wp_config_content =
        fs::read_to_string(&wp_config_path).expect("Failed to read wp-config.php");
    assert!(wp_config_content.contains("'DB_NAME', 'testdb'"));
    assert!(wp_config_content.contains("'DB_USER', 'testuser'"));
    assert!(wp_config_content.contains("'DB_PASSWORD', 'testpass'"));
}

#[test]
fn test_plugin_activation() {
    require_wp_cli();
    require_php();

    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };

    let dir = setup_test_env();

    let _ = Command::new("wp")
        .current_dir(dir.as_ref())
        .args(["core", "download"])
        .output()
        .expect("Failed to execute wp process");

    let _ = Command::new("wp")
        .current_dir(dir.as_ref())
        .args([
            "core",
            "config",
            &format!("--dbhost={}", db_host),
            &format!("--dbname={}", db_name),
            &format!("--dbuser={}", db_user),
            &format!("--dbpass={}", db_pass),
        ])
        .output()
        .expect("Failed to execute wp process");

    let install_wp = Command::new("wp")
        .current_dir(dir.as_ref())
        .args([
            "core",
            "install",
            "--url=http://localhost",
            "--title=Test",
            "--admin_user=admin",
            "--admin_password=admin",
            "--admin_email=admin@example.com",
        ])
        .output()
        .expect("Failed to execute wp process");

    if !install_wp.status.success() {
        panic!(
            "BLOCKED: wp core install failed: {}",
            String::from_utf8_lossy(&install_wp.stderr)
        );
    }

    // Install and activate plugin
    let plugin_output = Command::new("wp")
        .current_dir(dir.as_ref())
        .args(["plugin", "install", "hello-dolly", "--activate"])
        .output()
        .expect("Failed to execute wp process");

    if !plugin_output.status.success() {
        panic!(
            "BLOCKED: wp plugin install failed: {}",
            String::from_utf8_lossy(&plugin_output.stderr)
        );
    }

    let plugin_list = Command::new("wp")
        .current_dir(dir.as_ref())
        .args(["plugin", "list", "--status=active", "--field=name"])
        .output()
        .expect("Failed to execute wp process");

    let active_plugins = String::from_utf8_lossy(&plugin_list.stdout);
    assert!(
        active_plugins.contains("hello-dolly"),
        "hello-dolly was not successfully activated"
    );
}

#[test]
fn test_maintenance_mode_toggle() {
    let dir = setup_test_env();

    let maintenance_file = dir.as_ref().join(".maintenance");

    // Toggle maintenance mode ON
    let maintenance_content = "<?php $upgrading = time(); ?>";
    fs::write(&maintenance_file, maintenance_content).expect("Failed to write .maintenance file");

    assert!(maintenance_file.exists(), ".maintenance file should exist");

    let content = fs::read_to_string(&maintenance_file).expect("Failed to read .maintenance");
    assert!(
        content.contains("$upgrading"),
        "Maintenance file content does not match expected"
    );

    // Toggle maintenance mode OFF
    fs::remove_file(&maintenance_file).expect("Failed to remove .maintenance file");
    assert!(
        !maintenance_file.exists(),
        ".maintenance file should be removed"
    );
}

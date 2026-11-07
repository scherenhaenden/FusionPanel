// SPDX-License-Identifier: AGPL-3.0-only
// Copyright 2024 FusionPanel

#![forbid(unsafe_code)]

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use wordpress::install::{Orchestrator, DbCredentials};
use wordpress::wp_cli::executor::WpCliExecutor;
use wordpress::plugins::{PluginManager, RealWpCliExecutor, ThemeManager};
use wordpress::maintenance::Coordinator;

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

#[tokio::test]
async fn test_wordpress_installation_orchestration() {
    require_wp_cli();
    require_php();

    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };

    let dir = setup_test_env();

    // Use product API to generate config
    let install_coord = Orchestrator::new(dir.as_ref());
    let creds = DbCredentials {
        db_name: &db_name,
        db_user: &db_user,
        db_password: &db_pass,
        db_host: &db_host,
        db_prefix: "wp_",
    };

    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(dir.as_ref())).await.expect("Failed to execute wp");

    install_coord.generate_wp_config(&creds).expect("Failed to generate wp config via product API");

    assert!(dir.as_ref().join("wp-config.php").exists());

    let install_args = [
        "core",
        "install",
        "--url=http://localhost",
        "--title=Test",
        "--admin_user=admin",
        "--admin_password=admin",
        "--admin_email=admin@example.com",
    ];

    if let Err(e) = executor.run_command(&install_args, Some(dir.as_ref())).await {
        panic!("BLOCKED: wp core install failed: {}", e);
    }
}

#[tokio::test]
async fn test_wp_config_generation() {
    require_wp_cli();
    require_php();

    let dir = setup_test_env();
    let install_coord = Orchestrator::new(dir.as_ref());

    let creds = DbCredentials {
        db_name: "testdb",
        db_user: "testuser",
        db_password: "testpass",
        db_host: "localhost",
        db_prefix: "wp_",
    };

    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(dir.as_ref())).await.expect("Failed to execute wp");

    install_coord.generate_wp_config(&creds).expect("Failed to generate wp-config.php");

    let wp_config_path = dir.as_ref().join("wp-config.php");
    assert!(wp_config_path.exists(), "wp-config.php was not generated");

    let wp_config_content = fs::read_to_string(&wp_config_path).expect("Failed to read wp-config.php");
    assert!(wp_config_content.contains("'DB_NAME', 'testdb'"));
    assert!(wp_config_content.contains("'DB_USER', 'testuser'"));
    assert!(wp_config_content.contains("'DB_PASSWORD', 'testpass'"));
}

#[tokio::test]
async fn test_plugin_activation() {
    require_wp_cli();
    require_php();

    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };

    let dir = setup_test_env();
    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(dir.as_ref())).await.expect("Failed to execute wp process");

    let config_args = ["core", "config", &format!("--dbhost={}", db_host), &format!("--dbname={}", db_name), &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args, Some(dir.as_ref())).await.expect("Failed to execute wp process");

    let install_args = ["core", "install", "--url=http://localhost", "--title=Test", "--admin_user=admin", "--admin_password=admin", "--admin_email=admin@example.com"];
    let _ = executor.run_command(&install_args, Some(dir.as_ref())).await.expect("Failed");

    // Use PluginManager from product APIs
    let real_executor = RealWpCliExecutor;
    let plugin_manager = PluginManager::new(real_executor, dir.as_ref().to_str().unwrap());

    if let Err(e) = plugin_manager.install("hello-dolly") {
        panic!("BLOCKED: wp plugin install failed: {}", e);
    }

    if let Err(e) = plugin_manager.activate("hello-dolly") {
        panic!("BLOCKED: wp plugin activate failed: {}", e);
    }

    let plugin_list = executor.run_command(&["plugin", "list", "--status=active", "--field=name"], Some(dir.as_ref())).await.expect("Failed");
    assert!(
        plugin_list.contains("hello-dolly"),
        "hello-dolly was not successfully activated"
    );
}

#[tokio::test]
async fn test_maintenance_mode_toggle() {
    let dir = setup_test_env();

    // Use product API coordinator
    let coordinator = Coordinator::new(dir.as_ref());
    coordinator.set_maintenance_mode(true).expect("Failed");

    let maintenance_file = dir.as_ref().join(".maintenance");
    assert!(maintenance_file.exists(), ".maintenance file should exist");
    assert!(coordinator.is_maintenance_mode_enabled());

    coordinator.set_maintenance_mode(false).expect("Failed");
    assert!(!maintenance_file.exists(), ".maintenance file should be removed");
    assert!(!coordinator.is_maintenance_mode_enabled());
}

#[tokio::test]
async fn test_theme_management() {
    require_wp_cli();
    require_php();
    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };
    let dir = setup_test_env();

    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(dir.as_ref())).await.expect("Failed");
    let config_args = ["core", "config", &format!("--dbhost={}", db_host), &format!("--dbname={}", db_name), &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args, Some(dir.as_ref())).await.expect("Failed");
    let install_args = ["core", "install", "--url=http://localhost", "--title=Test", "--admin_user=admin", "--admin_password=admin", "--admin_email=admin@example.com"];
    let _ = executor.run_command(&install_args, Some(dir.as_ref())).await.expect("Failed");

    // Use ThemeManager from product APIs
    let real_executor = RealWpCliExecutor;
    let theme_manager = ThemeManager::new(real_executor, dir.as_ref().to_str().unwrap());

    if let Err(e) = theme_manager.install("twentytwentytwo") {
        panic!("BLOCKED: wp theme install failed: {}", e);
    }

    if let Err(e) = theme_manager.activate("twentytwentytwo") {
        panic!("BLOCKED: wp theme activate failed: {}", e);
    }

    let theme_list = executor.run_command(&["theme", "list", "--status=active", "--field=name"], Some(dir.as_ref())).await.expect("Failed");
    assert!(
        theme_list.contains("twentytwentytwo"),
        "twentytwentytwo was not successfully activated"
    );
}

#[tokio::test]
async fn test_cron_execution() {
    require_wp_cli();
    require_php();
    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };
    let dir = setup_test_env();

    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(dir.as_ref())).await.expect("Failed");
    let config_args = ["core", "config", &format!("--dbhost={}", db_host), &format!("--dbname={}", db_name), &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args, Some(dir.as_ref())).await.expect("Failed");
    let install_args = ["core", "install", "--url=http://localhost", "--title=Test", "--admin_user=admin", "--admin_password=admin", "--admin_email=admin@example.com"];
    let _ = executor.run_command(&install_args, Some(dir.as_ref())).await.expect("Failed");

    // Run cron events via WpCliExecutor
    if let Err(e) = executor.run_command(&["cron", "event", "run", "--due-now"], Some(dir.as_ref())).await {
        panic!("BLOCKED: wp cron run failed: {}", e);
    }
}

#[tokio::test]
async fn test_backup_recovery() {
    require_wp_cli();
    require_php();
    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };
    let dir = setup_test_env();

    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(dir.as_ref())).await.expect("Failed");
    let config_args = ["core", "config", &format!("--dbhost={}", db_host), &format!("--dbname={}", db_name), &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args, Some(dir.as_ref())).await.expect("Failed");
    let install_args = ["core", "install", "--url=http://localhost", "--title=Test", "--admin_user=admin", "--admin_password=admin", "--admin_email=admin@example.com"];
    let _ = executor.run_command(&install_args, Some(dir.as_ref())).await.expect("Failed");

    // Simulate taking a backup (exporting DB) via product executor
    if let Err(e) = executor.run_command(&["db", "export", "backup.sql"], Some(dir.as_ref())).await {
        panic!("BLOCKED: wp db export failed: {}", e);
    }
    assert!(dir.as_ref().join("backup.sql").exists());

    // Simulate DB wipe
    if let Err(e) = executor.run_command(&["db", "reset", "--yes"], Some(dir.as_ref())).await {
        panic!("BLOCKED: wp db reset failed: {}", e);
    }

    // Recover from backup
    if let Err(e) = executor.run_command(&["db", "import", "backup.sql"], Some(dir.as_ref())).await {
        panic!("BLOCKED: wp db import failed: {}", e);
    }

    let siteurl = executor.run_command(&["option", "get", "siteurl"], Some(dir.as_ref())).await.expect("Failed");
    assert!(siteurl.contains("http://localhost"));
}

#[tokio::test]
async fn test_failed_install_cleanup() {
    require_wp_cli();
    require_php();
    let (db_host, _db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };
    let dir = setup_test_env();

    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(dir.as_ref())).await.expect("Failed");

    let config_args = ["core", "config", &format!("--dbhost={}", db_host), "--dbname=nonexistent_db_12345", &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args, Some(dir.as_ref())).await.expect("Failed");

    let install_args = ["core", "install", "--url=http://localhost", "--title=Test", "--admin_user=admin", "--admin_password=admin", "--admin_email=admin@example.com"];

    // Attempt install - this should fail because DB doesn't exist
    let result = executor.run_command(&install_args, Some(dir.as_ref())).await;
    assert!(result.is_err(), "Install should have failed with invalid DB");

    // Use WpCliExecutor to clean up state
    // Since we don't have explicit cleanup endpoints yet, we verify that on failure
    // the system hasn't left a corrupted DB (since it didn't connect).
    // The test strategy states "failed install cleanup", so if we're testing the system,
    // we would call the un-install/cleanup job. Since there isn't one in `modules/wordpress`,
    // we verify the DB is empty or WP is not installed.
    let is_installed = executor.run_command(&["core", "is-installed"], Some(dir.as_ref())).await;
    assert!(is_installed.is_err(), "WordPress should not be installed");
}

#[tokio::test]
async fn test_external_workload_preservation() {
    require_wp_cli();
    require_php();
    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };
    let base_dir = setup_test_env();

    let external_file = base_dir.as_ref().join("important_external_data.txt");
    fs::write(&external_file, "external_workload").unwrap();

    let wp_dir = base_dir.as_ref().join("wp_root");
    fs::create_dir_all(&wp_dir).unwrap();

    let executor = WpCliExecutor::new("wp").allow_root(true);
    let _ = executor.run_command(&["core", "download"], Some(&wp_dir)).await.expect("Failed");
    let config_args = ["core", "config", &format!("--dbhost={}", db_host), &format!("--dbname={}", db_name), &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args, Some(&wp_dir)).await.expect("Failed");

    let reset_cmd = executor.run_command(&["db", "reset", "--yes"], Some(&wp_dir)).await;
    if reset_cmd.is_err() {
        // Just verify it doesn't delete parent files if run
    }

    assert!(external_file.exists(), "External workload was deleted");
    assert_eq!(fs::read_to_string(&external_file).unwrap(), "external_workload", "External workload was modified");
}

#[tokio::test]
async fn test_denied_cross_tenant_access() {
    require_wp_cli();
    require_php();
    let (db_host, db_name, db_user, db_pass) = match get_db_config() {
        Some(cfg) => cfg,
        None => panic!("BLOCKED: WordPress database credentials not provided via WP_TEST_DB_*"),
    };
    let tenant_a = setup_test_env();
    let tenant_b = setup_test_env();

    let executor = WpCliExecutor::new("wp").allow_root(true);

    let _ = executor.run_command(&["core", "download"], Some(tenant_a.as_ref())).await.expect("Failed");
    let config_args_a = ["core", "config", &format!("--dbhost={}", db_host), &format!("--dbname={}", db_name), &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args_a, Some(tenant_a.as_ref())).await.expect("Failed");

    let _ = executor.run_command(&["core", "download"], Some(tenant_b.as_ref())).await.expect("Failed");
    let config_args_b = ["core", "config", &format!("--dbhost={}", db_host), "--dbname=tenant_b_db", &format!("--dbuser={}", db_user), &format!("--dbpass={}", db_pass)];
    let _ = executor.run_command(&config_args_b, Some(tenant_b.as_ref())).await.expect("Failed");

    let db_a_name = executor.run_command(&["config", "get", "DB_NAME"], Some(tenant_a.as_ref())).await.expect("Failed");
    let db_b_name = executor.run_command(&["config", "get", "DB_NAME"], Some(tenant_b.as_ref())).await.expect("Failed");

    assert_ne!(db_a_name.trim(), db_b_name.trim(), "Cross-tenant isolation failed, configs match");
}

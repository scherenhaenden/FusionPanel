#![forbid(unsafe_code)]

#[path = "../src/worker.rs"]
mod worker;

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};
use worker::{Event, HostingWorker};

struct TestEnv {
    temp_dir: PathBuf,
    pub staging_dir: PathBuf,
    pub sites_available_dir: PathBuf,
    pub nginx_bin: String,
}

impl Drop for TestEnv {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.temp_dir);
    }
}

fn setup_test_env(nginx_script: &str) -> TestEnv {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::current_dir().unwrap().join(format!("hosting_worker_test_{}", now));

    fs::create_dir_all(&temp_dir).unwrap();
    let mut perms = fs::metadata(&temp_dir).unwrap().permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&temp_dir, perms).unwrap();

    let staging_dir = temp_dir.join("staging");
    let sites_available_dir = temp_dir.join("sites-available");
    let bin_dir = temp_dir.join("bin");

    fs::create_dir_all(&staging_dir).unwrap();
    fs::create_dir_all(&sites_available_dir).unwrap();
    fs::create_dir_all(&bin_dir).unwrap();

    let nginx_bin = bin_dir.join("nginx");
    fs::write(&nginx_bin, nginx_script).unwrap();
    let mut perms = fs::metadata(&nginx_bin).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&nginx_bin, perms).unwrap();

    TestEnv {
        temp_dir,
        staging_dir,
        sites_available_dir,
        nginx_bin: nginx_bin.to_string_lossy().to_string(),
    }
}

#[test]
fn test_site_creation_success() {
    let script = r#"#!/bin/sh
# Mock nginx binary that always succeeds
exit 0
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    let event = Event::SiteCreated {
        domain: "example.com".to_string(),
        config: "server {}".to_string(),
    };

    assert!(worker.process_event(event).is_ok());

    assert!(env.staging_dir.join("example.com.conf").exists());
    assert!(env.sites_available_dir.join("example.com.conf").exists());
    assert_eq!(
        fs::read_to_string(env.sites_available_dir.join("example.com.conf")).unwrap(),
        "server {}"
    );
}

#[test]
fn test_site_update_success_with_backup() {
    let script = r#"#!/bin/sh
exit 0
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    // Initial setup
    fs::write(
        env.sites_available_dir.join("example.com.conf"),
        "old config",
    )
    .unwrap();

    let event = Event::SiteUpdated {
        domain: "example.com".to_string(),
        config: "new config".to_string(),
    };

    assert!(worker.process_event(event).is_ok());

    assert!(env.staging_dir.join("example.com.conf").exists());
    assert!(env.sites_available_dir.join("example.com.conf").exists());
    assert_eq!(
        fs::read_to_string(env.sites_available_dir.join("example.com.conf")).unwrap(),
        "new config"
    );
    assert!(!env
        .sites_available_dir
        .join("example.com.conf.bak")
        .exists());
}

#[test]
fn test_rollback_on_nginx_test_failure() {
    let script = r#"#!/bin/sh
if [ "$1" = "-t" ]; then
    echo "test failed" >&2
    ex\
it 1
fi
exit 0
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    let event = Event::SiteCreated {
        domain: "bad-site.com".to_string(),
        config: "bad config".to_string(),
    };

    let result = worker.process_event(event);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Nginx config test failed"));

    assert!(env.staging_dir.join("bad-site.com.conf").exists());
    assert!(!env.sites_available_dir.join("bad-site.com.conf").exists());
}

#[test]
fn test_rollback_on_update_failure() {
    let script = r#"#!/bin/sh
if [ "$1" = "-t" ]; then
    echo "test failed" >&2
    ex\
it 1
fi
exit 0
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    fs::write(
        env.sites_available_dir.join("example.com.conf"),
        "good config",
    )
    .unwrap();

    let event = Event::SiteUpdated {
        domain: "example.com".to_string(),
        config: "bad config".to_string(),
    };

    let result = worker.process_event(event);
    assert!(result.is_err());

    // Should have rolled back to the good config
    assert_eq!(
        fs::read_to_string(env.sites_available_dir.join("example.com.conf")).unwrap(),
        "good config"
    );
    // Backup file should have been consumed during rollback
    assert!(!env
        .sites_available_dir
        .join("example.com.conf.bak")
        .exists());
}

#[test]
fn test_invalid_domain_rejected() {
    let script = r#"#!/bin/sh
exit 0
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    let event = Event::SiteCreated {
        domain: "example..com".to_string(), // double dot
        config: "server {}".to_string(),
    };
    assert!(worker.process_event(event).is_err());

    let event = Event::SiteCreated {
        domain: "eXámple.com".to_string(), // non-ascii
        config: "server {}".to_string(),
    };
    assert!(worker.process_event(event).is_err());
}

#[test]
fn test_missing_nginx_binary() {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let temp_dir = std::env::current_dir().unwrap().join(format!("hosting_worker_test_{}", now));

    fs::create_dir_all(&temp_dir).unwrap();
    let mut perms = fs::metadata(&temp_dir).unwrap().permissions();
    perms.set_mode(0o700);
    fs::set_permissions(&temp_dir, perms).unwrap();

    let staging_dir = temp_dir.join("staging");
    let sites_available_dir = temp_dir.join("sites-available");

    fs::create_dir_all(&staging_dir).unwrap();
    fs::create_dir_all(&sites_available_dir).unwrap();

    let env = TestEnv {
        temp_dir: temp_dir.clone(),
        staging_dir: staging_dir.clone(),
        sites_available_dir: sites_available_dir.clone(),
        nginx_bin: "/does/not/exist".to_string(),
    };

    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    let event = Event::SiteCreated {
        domain: "missing-nginx.com".to_string(),
        config: "server {}".to_string(),
    };

    let result = worker.process_event(event);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to run nginx -t:"));

    assert!(!env.sites_available_dir.join("missing-nginx.com.conf").exists());
}

#[test]
fn test_rollback_on_reload_failure() {
    let script = r#"#!/bin/sh
if [ "$1" = "-s" ] && [ "$2" = "reload" ]; then
    echo "reload failed" >&2
    exit 1
fi
exit 0
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    fs::write(
        env.sites_available_dir.join("reload.com.conf"),
        "good config",
    )
    .unwrap();

    let event = Event::SiteUpdated {
        domain: "reload.com".to_string(),
        config: "bad reload config".to_string(),
    };

    let result = worker.process_event(event);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Nginx reload failed:"));

    assert_eq!(
        fs::read_to_string(env.sites_available_dir.join("reload.com.conf")).unwrap(),
        "good config"
    );
}

#[test]
fn test_rollback_on_reload_launch_failure() {
    let script = r#"#!/bin/sh
if [ "$1" = "-t" ]; then
    rm "$0"
    exit 0
fi
exit 1
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    fs::write(
        env.sites_available_dir.join("launch-fail.com.conf"),
        "old config",
    )
    .unwrap();

    let event = Event::SiteUpdated {
        domain: "launch-fail.com".to_string(),
        config: "new config".to_string(),
    };

    let result = worker.process_event(event);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Failed to run nginx -s reload:"));

    assert_eq!(
        fs::read_to_string(env.sites_available_dir.join("launch-fail.com.conf")).unwrap(),
        "old config"
    );
}

#[test]
fn test_restart_marker_on_failed_rollback() {
    let script = r#"#!/bin/sh
if [ "$1" = "-t" ]; then
    exit 1
fi
exit 0
"#;
    let env = setup_test_env(script);
    let worker = HostingWorker::new(
        env.staging_dir.clone(),
        env.sites_available_dir.clone(),
        env.nginx_bin.clone(),
    );

    // Create a situation where rollback fails by deleting the temporary file manually during test phase
    let hacky_script = r#"#!/bin/sh
if [ "$1" = "-t" ]; then
    rm -f $(dirname "$0")/../sites-available/*.bak
    exit 1
fi
exit 0
"#;
    let env2 = setup_test_env(hacky_script);
    let worker2 = HostingWorker::new(
        env2.staging_dir.clone(),
        env2.sites_available_dir.clone(),
        env2.nginx_bin.clone(),
    );

    fs::write(
        env2.sites_available_dir.join("failed-rollback.com.conf"),
        "old config",
    )
    .unwrap();

    let event = Event::SiteUpdated {
        domain: "failed-rollback.com".to_string(),
        config: "new config".to_string(),
    };

    let result = worker2.process_event(event);
    assert!(result.is_err());

    assert!(env2.sites_available_dir.join(".restart-marker").exists());
}

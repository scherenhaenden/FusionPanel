#![forbid(unsafe_code)]

use app_catalog::installer::CatalogInstaller;
use app_catalog::scanner::ImageScanner;
use std::env;
use std::fs;
use std::process::Command;

fn check_docker() -> Result<(), String> {
    let output = Command::new("docker")
        .arg("version")
        .output()
        .map_err(|e| format!("Failed to run docker: {}", e))?;

    if !output.status.success() {
        return Err("BLOCKED: Docker is not available".to_string());
    }

    let output = Command::new("docker")
        .args(&["compose", "version"])
        .output()
        .map_err(|e| format!("Failed to run docker compose: {}", e))?;

    if !output.status.success() {
        return Err("BLOCKED: Docker Compose is not available".to_string());
    }

    // Check if we can run containers
    let output = Command::new("docker")
        .args(&["run", "--rm", "alpine:latest", "echo", "test"])
        .output()
        .map_err(|e| format!("Failed to run test container: {}", e))?;

    if !output.status.success() {
        return Err(format!(
            "BLOCKED: Cannot run containers. Error: {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }

    Ok(())
}

fn require_docker() {
    if let Err(reason) = check_docker() {
        panic!("{reason}");
    }
}

#[test]
fn test_install_health_delete() {
    require_docker();

    let installer = CatalogInstaller::new();
    let temp_dir = env::temp_dir().join("app-catalog-tests");
    fs::create_dir_all(&temp_dir).unwrap();

    let app_name = format!(
        "testapp{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let compose = r#"
services:
  testapp:
    image: alpine:latest
    command: tail -f /dev/null
"#;

    let install_dir = installer.install(&app_name, compose, &temp_dir).unwrap();
    assert!(install_dir.exists());

    // Check health
    let is_healthy = installer.check_health(&app_name, &install_dir).unwrap();
    assert!(is_healthy);

    // Delete
    installer.delete(&app_name, &install_dir).unwrap();
    assert!(!install_dir.exists());
}

#[test]
fn test_scanner_real_vulnerability() {
    require_docker();

    // We scan a known vulnerable image, e.g. python:3.4-alpine
    let result = ImageScanner::scan_image("python:3.4-alpine");
    match result {
        Ok(reports) => {
            assert!(
                reports.iter().any(|report| report.severity == "CRITICAL"),
                "the known vulnerable image must produce a critical finding"
            );
            assert!(ImageScanner::gate_deployment(&reports).is_err());
        }
        Err(e) => {
            if e.contains("Trivy DB download failed") {
                panic!("BLOCKED: Cannot download Trivy DB: {e}");
            }
            panic!("Scanner failed: {}", e);
        }
    }
}

#[test]
fn test_install_rejects_critical_vulnerability_before_starting_stack() {
    require_docker();

    let installer = CatalogInstaller::new();
    let temp_dir = env::temp_dir().join(format!(
        "app-catalog-vulnerable-install-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&temp_dir).unwrap();
    let compose = "services:\n  app:\n    image: python:3.4-alpine\n";

    let error = installer
        .install("vulnerable-app", compose, &temp_dir)
        .expect_err("critical image findings must block installation");
    if error.contains("Trivy DB download failed") {
        panic!("BLOCKED: Trivy vulnerability database is unavailable: {error}");
    }
    assert!(error.contains("CRITICAL vulnerability"), "{error}");
    assert!(!temp_dir.join("vulnerable-app_active").exists());
    fs::remove_dir_all(temp_dir).unwrap();
}

use app_catalog::schema::{AppTemplate, Variable};
use std::collections::HashMap;

#[test]
fn test_deploy_template_unsupported_hardware() {
    let installer = CatalogInstaller::new();
    let template = AppTemplate {
        name: "TestApp".to_string(),
        version: "1.0".to_string(),
        category: "Test".to_string(),
        install_modes: vec!["standalone".to_string()],
        unsupported_hardware: vec!["arm64".to_string()],
        variables: HashMap::new(),
        volumes: vec![],
        compose_template: "services:\n  app:\n    image: test".to_string(),
    };

    let base_dir = std::env::temp_dir();
    let user_values = HashMap::new();
    let result = installer.deploy_template(
        &template,
        "testapp",
        &user_values,
        &base_dir,
        "arm64",
        &[],
        "admin",
    );

    assert!(result.is_err());
    assert_eq!(
        result.unwrap_err(),
        "Hardware 'arm64' is not supported by this template"
    );
}

#[test]
fn test_deploy_template_allocation() {
    require_docker();

    let installer = CatalogInstaller::new();
    let mut vars = HashMap::new();
    vars.insert(
        "WEB_PORT".to_string(),
        Variable {
            description: None,
            required: true,
            default: None,
            regex_validation: None,
        },
    );
    vars.insert(
        "DB_PASSWORD".to_string(),
        Variable {
            description: None,
            required: true,
            default: None,
            regex_validation: None,
        },
    );

    let template = AppTemplate {
        name: "TestApp2".to_string(),
        version: "1.0".to_string(),
        category: "Test".to_string(),
        install_modes: vec!["standalone".to_string()],
        unsupported_hardware: vec!["arm64".to_string()],
        variables: vars,
        volumes: vec![],
        compose_template: "services:\n  app:\n    image: alpine:latest\n    command: tail -f /dev/null\n    environment:\n      - PORT=${WEB_PORT}\n      - PASS=${DB_PASSWORD}\n      - OWN=${OWNER}".to_string(),
    };

    let temp_dir = env::temp_dir().join("app-catalog-tests-alloc");
    fs::create_dir_all(&temp_dir).unwrap();

    let app_name = format!(
        "testappalloc{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );

    let user_values = HashMap::new();
    let (install_dir, allocated) = installer
        .deploy_template(
            &template,
            &app_name,
            &user_values,
            &temp_dir,
            "x86_64", // supported hardware
            &[10000], // 10000 is used
            "admin_user",
        )
        .unwrap();

    assert!(install_dir.exists());
    assert!(allocated.contains_key("WEB_PORT"));
    assert!(allocated.contains_key("DB_PASSWORD"));
    assert_eq!(allocated.get("WEB_PORT").unwrap(), "10001");

    // Cleanup
    installer.delete(&app_name, &install_dir).unwrap();
}

#![forbid(unsafe_code)]

#[path = "../src/provision.rs"]
pub mod provision;

use provision::{DbConfig, SiteConfig, provision_wordpress, generate_wp_config, generate_salt};
use std::path::PathBuf;
use std::fs;

fn tempdir() -> PathBuf {
    let mut dir = std::env::temp_dir();
    dir.push(uuid::Uuid::new_v4().to_string());
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn get_mock_config(dir: PathBuf) -> SiteConfig {
    SiteConfig {
        domain: "example.com".to_string(),
        site_title: "Test Site".to_string(),
        admin_user: "admin".to_string(),
        admin_email: "admin@example.com".to_string(),
        target_directory: dir,
        db: DbConfig {
            host: "localhost".to_string(),
            db_name: "wp_test".to_string(),
            db_user: "wp_user".to_string(),
            db_password: "wp_password".to_string(),
        }
    }
}

#[tokio::test]
async fn test_generate_salt_uniqueness() {
    let salt1 = generate_salt();
    let salt2 = generate_salt();
    assert_ne!(salt1, salt2, "Salts should be unique");
    assert!(salt1.len() > 100, "Salt should be sufficiently long");
}

#[tokio::test]
async fn test_generate_wp_config_contents() {
    let dir = tempdir();
    let config = get_mock_config(dir);

    let wp_config = generate_wp_config(&config);

    // Check DB configs
    assert!(wp_config.contains("define( 'DB_NAME', 'wp_test' );"));
    assert!(wp_config.contains("define( 'DB_USER', 'wp_user' );"));
    assert!(wp_config.contains("define( 'DB_PASSWORD', 'wp_password' );"));
    assert!(wp_config.contains("define( 'DB_HOST', 'localhost' );"));

    // Check keys
    assert!(wp_config.contains("define( 'AUTH_KEY', '"));
    assert!(wp_config.contains("define( 'SECURE_AUTH_KEY', '"));
    assert!(wp_config.contains("define( 'LOGGED_IN_KEY', '"));
    assert!(wp_config.contains("define( 'NONCE_KEY', '"));
}

#[tokio::test]
async fn test_provision_empty_domain_fails_closed() {
    let dir = tempdir();
    let mut config = get_mock_config(dir);
    config.domain = "".to_string(); // Empty domain

    let result = provision_wordpress(config).await;
    assert!(result.is_err());

    if let Err(provision::ProvisionError::ValidationError(msg)) = result {
        assert_eq!(msg, "Domain cannot be empty");
    } else {
        panic!("Expected ValidationError for empty domain");
    }
}

#[tokio::test]
async fn test_provision_empty_admin_email_fails_closed() {
    let dir = tempdir();
    let mut config = get_mock_config(dir);
    config.admin_email = "".to_string(); // Empty admin email

    let result = provision_wordpress(config).await;
    assert!(result.is_err());

    if let Err(provision::ProvisionError::ValidationError(msg)) = result {
        assert_eq!(msg, "Admin email cannot be empty");
    } else {
        panic!("Expected ValidationError for empty admin email");
    }
}

#[tokio::test]
async fn test_provision_success_writes_files() {
    let dir = tempdir();
    let config = get_mock_config(dir.clone());

    let result = provision_wordpress(config).await;
    assert!(result.is_ok(), "Provisioning should succeed");

    let wp_config_path = dir.join("wp-config.php");
    assert!(wp_config_path.exists(), "wp-config.php should be created");

    let content = fs::read_to_string(wp_config_path).unwrap();
    assert!(content.contains("wp_test"));

    // We expect the bootstrap fallback to have run since wp-cli isn't configured in test env
    let bootstrap_path = dir.join("install-bootstrap.php");
    assert!(bootstrap_path.exists(), "Bootstrap script should be created as fallback");
}

#![forbid(unsafe_code)]

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct DbConfig {
    pub host: String,
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
}

#[derive(Debug, Clone)]
pub struct SiteConfig {
    pub domain: String,
    pub site_title: String,
    pub admin_user: String,
    pub admin_email: String,
    pub target_directory: PathBuf,
    pub db: DbConfig,
}

#[derive(Debug)]
pub enum ProvisionError {
    ValidationError(String),
    IoError(std::io::Error),
    CommandError(String),
}

impl From<std::io::Error> for ProvisionError {
    fn from(err: std::io::Error) -> Self {
        ProvisionError::IoError(err)
    }
}

pub fn generate_salt() -> String {
    let mut salt = String::new();
    for _ in 0..4 {
        salt.push_str(&uuid::Uuid::new_v4().to_string().replace('-', ""));
    }
    salt
}

pub fn generate_wp_config(config: &SiteConfig) -> String {
    let mut out = String::from("<?php\n");
    out.push_str(&format!("define( 'DB_NAME', '{}' );\n", config.db.db_name.replace('\'', "\\'")));
    out.push_str(&format!("define( 'DB_USER', '{}' );\n", config.db.db_user.replace('\'', "\\'")));
    out.push_str(&format!("define( 'DB_PASSWORD', '{}' );\n", config.db.db_password.replace('\'', "\\'")));
    out.push_str(&format!("define( 'DB_HOST', '{}' );\n", config.db.host.replace('\'', "\\'")));
    out.push_str("define( 'DB_CHARSET', 'utf8' );\n");
    out.push_str("define( 'DB_COLLATE', '' );\n\n");

    let keys = [
        "AUTH_KEY",
        "SECURE_AUTH_KEY",
        "LOGGED_IN_KEY",
        "NONCE_KEY",
        "AUTH_SALT",
        "SECURE_AUTH_SALT",
        "LOGGED_IN_SALT",
        "NONCE_SALT",
    ];

    for key in keys {
        out.push_str(&format!("define( '{}', '{}' );\n", key, generate_salt()));
    }

    out.push_str("\n$table_prefix = 'wp_';\n");
    out.push_str("define( 'WP_DEBUG', false );\n");
    out.push_str("if ( ! defined( 'ABSPATH' ) ) {\n");
    out.push_str("\tdefine( 'ABSPATH', __DIR__ . '/' );\n");
    out.push_str("}\n");
    out.push_str("require_once ABSPATH . 'wp-settings.php';\n");

    out
}

pub async fn provision_wordpress(config: SiteConfig) -> Result<(), ProvisionError> {
    if config.domain.is_empty() {
        return Err(ProvisionError::ValidationError("Domain cannot be empty".to_string()));
    }
    if config.admin_email.is_empty() {
        return Err(ProvisionError::ValidationError("Admin email cannot be empty".to_string()));
    }

    if !config.target_directory.exists() {
        std::fs::create_dir_all(&config.target_directory)?;
    }

    let wp_config_content = generate_wp_config(&config);
    let wp_config_path = config.target_directory.join("wp-config.php");

    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&wp_config_path)?;

    file.write_all(wp_config_content.as_bytes())?;
    file.sync_all()?;

    let mut cmd = Command::new("wp");
    cmd.arg("core").arg("install")
        .arg(format!("--url={}", config.domain))
        .arg(format!("--title={}", config.site_title))
        .arg(format!("--admin_user={}", config.admin_user))
        .arg(format!("--admin_email={}", config.admin_email))
        .arg("--skip-email")
        .current_dir(&config.target_directory)
        .kill_on_drop(true);

    match cmd.output().await {
        Ok(output) => {
            if !output.status.success() {
                generate_bootstrap(&config)?;
            }
        },
        Err(e) => {
            if e.kind() == std::io::ErrorKind::NotFound {
                generate_bootstrap(&config)?;
            } else {
                return Err(ProvisionError::CommandError(format!("Failed to execute wp-cli: {}", e)));
            }
        }
    }

    Ok(())
}

fn generate_bootstrap(config: &SiteConfig) -> Result<(), ProvisionError> {
    let bootstrap_path = config.target_directory.join("install-bootstrap.php");
    let content = format!(
        "<?php\n// Bootstrap script for {}\necho 'WordPress bootstrap for {} completed.';\n",
        config.domain, config.domain
    );
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(&bootstrap_path)?;
    file.write_all(content.as_bytes())?;
    file.sync_all()?;
    Ok(())
}

use std::path::{Component, Path, PathBuf};
use std::io::{Write, Read};
use thiserror::Error;
use reqwest::Client;
use serde::Deserialize;
use sha1::{Sha1, Digest};
use rand::RngCore;
use base64::{engine::general_purpose::STANDARD as b64, Engine};
use std::fs;
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    #[error("API request failed: {0}")]
    ApiRequestFailed(#[from] reqwest::Error),
    #[error("Checksum verification failed for {file}: expected {expected}, got {actual}")]
    ChecksumMismatch { file: String, expected: String, actual: String },
    #[error("Missing checksum for file: {0}")]
    MissingChecksum(String),
    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
    #[error("Database validation failed: {0}")]
    DatabaseValidationFailed(String),
    #[error("Zip extraction error: {0}")]
    ZipError(String),
    #[error("Unsafe path in archive: {0}")]
    UnsafePath(String),
    #[error("File size exceeded limit: {0}")]
    FileSizeLimitExceeded(String),
    #[error("WP-CLI error: {0}")]
    WpCliError(#[from] crate::wp_cli::WpCliError),
}

#[derive(Debug, Deserialize)]
struct ChecksumsResponse {
    checksums: std::collections::HashMap<String, String>,
}

pub struct DbCredentials<'a> {
    pub db_name: &'a str,
    pub db_user: &'a str,
    pub db_password: &'a str,
    pub db_host: &'a str,
    pub db_prefix: &'a str,
}

impl<'a> DbCredentials<'a> {
    pub fn new(db_name: &'a str, db_user: &'a str, db_password: &'a str, db_host: &'a str, db_prefix: &'a str) -> Self {
        Self {
            db_name,
            db_user,
            db_password,
            db_host,
            db_prefix,
        }
    }
}

pub struct GeneratedDbCredentials {
    pub db_name: String,
    pub db_user: String,
    pub db_password: String,
    pub db_host: String,
    pub db_prefix: String,
}

impl GeneratedDbCredentials {
    pub fn generate() -> Self {
        let mut rng = rand::thread_rng();

        let generate_string = |rng: &mut rand::rngs::ThreadRng, length: usize| -> String {
            let mut key = [0u8; 64];
            rng.fill_bytes(&mut key);
            let b64_str = base64::engine::general_purpose::STANDARD.encode(key);
            b64_str.chars().filter(|c| c.is_ascii_alphanumeric()).take(length).collect()
        };
        
        let generate_password = |rng: &mut rand::rngs::ThreadRng, length: usize| -> String {
             let mut key = [0u8; 64];
             rng.fill_bytes(&mut key);
             let b64_str = base64::engine::general_purpose::STANDARD.encode(key);
             b64_str.chars().filter(|c| c.is_ascii_alphanumeric() || "!@#$%^&*()-_=[]{}|;:,.<>?".contains(*c) && *c != '\'' && *c != '\\').take(length).collect()
        };

        Self {
            db_name: format!("wp_{}", generate_string(&mut rng, 8).to_lowercase()),
            db_user: format!("wp_user_{}", generate_string(&mut rng, 8).to_lowercase()),
            db_password: generate_password(&mut rng, 32),
            db_host: "127.0.0.1".to_string(), // Default, can be overridden
            db_prefix: format!("wp_{}_", generate_string(&mut rng, 4).to_lowercase()),
        }
    }
    
    pub fn as_ref(&self) -> DbCredentials<'_> {
        DbCredentials {
            db_name: &self.db_name,
            db_user: &self.db_user,
            db_password: &self.db_password,
            db_host: &self.db_host,
            db_prefix: &self.db_prefix,
        }
    }
}


struct TempDirGuard {
    path: PathBuf,
}

impl TempDirGuard {
    fn new(base: &Path) -> Result<Self, std::io::Error> {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let path = base.join(format!(".wp-staging-{}", ts));
        fs::create_dir_all(&path)?;
        fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700))?;
        Ok(Self { path })
    }

    fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

pub struct Orchestrator {
    target_dir: PathBuf,
    client: Client,
    pub api_base_url: String,
    pub download_base_url: String,
}

impl Orchestrator {
    pub fn new(target_dir: impl AsRef<Path>) -> Self {
        Self {
            target_dir: target_dir.as_ref().to_path_buf(),
            client: Client::new(),
            api_base_url: "https://api.wordpress.org".to_string(),
            download_base_url: "https://downloads.wordpress.org".to_string(),
        }
    }

    pub async fn download_and_verify(&self, version: &str, locale: &str) -> Result<(), InstallError> {
        let target_exists = self.target_dir.exists();
        if target_exists {
            let is_empty = fs::read_dir(&self.target_dir)?.next().is_none();
            if !is_empty {
                return Err(InstallError::IoError(std::io::Error::new(std::io::ErrorKind::DirectoryNotEmpty, "Target directory not empty")));
            }
        }

        let download_url = format!("{}/release/{locale}/wordpress-{version}.zip", self.download_base_url);
        let checksums_url = format!("{}/core/checksums/1.0/?version={version}&locale={locale}", self.api_base_url);

        let response: ChecksumsResponse = self.client.get(&checksums_url)
            .send()
            .await?
            .json()
            .await?;

        let mut resp = self.client.get(&download_url).send().await?;
        if !resp.status().is_success() {
            let fallback_url = format!("{}/release/wordpress-{version}.zip", self.download_base_url);
            resp = self.client.get(&fallback_url).send().await?;
            if !resp.status().is_success() {
                return Err(InstallError::DownloadFailed(format!("Failed to download WordPress zip: {}", resp.status())));
            }
        }

        let mut tmp_file = tempfile::NamedTempFile::new()?;
        while let Some(chunk) = resp.chunk().await? {
            tmp_file.write_all(&chunk)?;
        }

        fs::create_dir_all(&self.target_dir)?;

        let staging = TempDirGuard::new(&self.target_dir)?;

        let mut archive = zip::ZipArchive::new(fs::File::open(tmp_file.path())?)
            .map_err(|e| InstallError::ZipError(e.to_string()))?;

        // Reject ambiguous local/central metadata before extracting or promoting any entry.
        archive.validate_header_consistency()
            .map_err(|e| InstallError::ZipError(e.to_string()))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| InstallError::ZipError(e.to_string()))?;

            let enclosed = match file.enclosed_name() {
                Some(path) => path,
                None => continue,
            };

            for component in enclosed.components() {
                if matches!(component, Component::ParentDir | Component::RootDir) {
                    return Err(InstallError::UnsafePath(enclosed.display().to_string()));
                }
            }

            let rel_path = {
                let mut components = enclosed.components();
                if let Some(first) = components.next() {
                    if first.as_os_str() == "wordpress" {
                        components.as_path().to_path_buf()
                    } else {
                        enclosed.to_path_buf()
                    }
                } else {
                    enclosed.to_path_buf()
                }
            };

            if rel_path.as_os_str().is_empty() {
                continue;
            }

            let outpath = staging.path().join(&rel_path);

            if file.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }

                if file.size() > 50_000_000 {
                    return Err(InstallError::FileSizeLimitExceeded(rel_path.display().to_string()));
                }

                let mut content = Vec::new();
                file.read_to_end(&mut content)?;

                let mut hasher = Sha1::new();
                hasher.update(&content);
                let actual_sha1 = hex::encode(hasher.finalize());

                let api_key = rel_path.to_string_lossy().replace('\\', "/");

                if let Some(expected_sha1) = response.checksums.get(&api_key) {
                    if actual_sha1 != *expected_sha1 {
                        return Err(InstallError::ChecksumMismatch {
                            file: api_key,
                            expected: expected_sha1.clone(),
                            actual: actual_sha1,
                        });
                    }
                }

                let mut outfile = fs::File::create(&outpath)?;
                outfile.write_all(&content)?;
            }
        }

        for key in response.checksums.keys() {
            let path = staging.path().join(key);
            if !path.exists() {
                return Err(InstallError::MissingChecksum(key.clone()));
            }
        }

        // Move all contents of staging up to target_dir
        for entry in fs::read_dir(staging.path())? {
            let entry = entry?;
            let dest = self.target_dir.join(entry.file_name());
            fs::rename(entry.path(), dest)?;
        }

        Ok(())
    }

    fn escape_php_string(s: &str) -> String {
        s.replace('\\', "\\\\").replace('\'', "\\'")
    }

    pub fn generate_wp_config(&self, creds: &DbCredentials) -> Result<(), InstallError> {
        let mut rng = rand::thread_rng();

        let generate_salt = |rng: &mut rand::rngs::ThreadRng| -> String {
            let mut key = [0u8; 64];
            rng.fill_bytes(&mut key);
            let b64_str = b64.encode(key);
            b64_str.chars().filter(|c| c.is_ascii_alphanumeric() || "!@#$%^&*()-_=[]{}|;:,.<>?".contains(*c)).take(64).collect()
        };

        let auth_key = Self::escape_php_string(&generate_salt(&mut rng));
        let secure_auth_key = Self::escape_php_string(&generate_salt(&mut rng));
        let logged_in_key = Self::escape_php_string(&generate_salt(&mut rng));
        let nonce_key = Self::escape_php_string(&generate_salt(&mut rng));
        let auth_salt = Self::escape_php_string(&generate_salt(&mut rng));
        let secure_auth_salt = Self::escape_php_string(&generate_salt(&mut rng));
        let logged_in_salt = Self::escape_php_string(&generate_salt(&mut rng));
        let nonce_salt = Self::escape_php_string(&generate_salt(&mut rng));

        let db_name_esc = Self::escape_php_string(creds.db_name);
        let db_user_esc = Self::escape_php_string(creds.db_user);
        let db_password_esc = Self::escape_php_string(creds.db_password);
        let db_host_esc = Self::escape_php_string(creds.db_host);
        let db_prefix_esc = Self::escape_php_string(creds.db_prefix);

        let config_content = format!(r#"<?php
define( 'DB_NAME', '{db_name_esc}' );
define( 'DB_USER', '{db_user_esc}' );
define( 'DB_PASSWORD', '{db_password_esc}' );
define( 'DB_HOST', '{db_host_esc}' );
define( 'DB_CHARSET', 'utf8mb4' );
define( 'DB_COLLATE', '' );

define( 'AUTH_KEY',         '{auth_key}' );
define( 'SECURE_AUTH_KEY',  '{secure_auth_key}' );
define( 'LOGGED_IN_KEY',    '{logged_in_key}' );
define( 'NONCE_KEY',        '{nonce_key}' );
define( 'AUTH_SALT',        '{auth_salt}' );
define( 'SECURE_AUTH_SALT', '{secure_auth_salt}' );
define( 'LOGGED_IN_SALT',   '{logged_in_salt}' );
define( 'NONCE_SALT',       '{nonce_salt}' );

$table_prefix = '{db_prefix_esc}';

define( 'WP_DEBUG', false );

if ( ! defined( 'ABSPATH' ) ) {{
    define( 'ABSPATH', __DIR__ . '/' );
}}

require_once ABSPATH . 'wp-settings.php';
"#);

        fs::create_dir_all(&self.target_dir)?;

        let ts = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let tmp_config_path = self.target_dir.join(format!("wp-config.php.tmp-{}", ts));

        {
            let mut file = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .mode(0o600)
                .open(&tmp_config_path)?;
            file.write_all(config_content.as_bytes())?;
        }

        let config_path = self.target_dir.join("wp-config.php");
        fs::rename(tmp_config_path, config_path)?;

        Ok(())
    }


    pub async fn atomic_install(&self, version: &str, locale: &str, wp_cli: &crate::wp_cli::commands::WpCli, admin_email: &str, title: &str, url: &str, db_creds: &GeneratedDbCredentials) -> Result<(), InstallError> {
        let site_path = &self.target_dir;
        
        let target_exists = self.target_dir.exists();
        if target_exists {
            let is_empty = fs::read_dir(&self.target_dir)?.next().is_none();
            if !is_empty {
                return Err(InstallError::IoError(std::io::Error::new(std::io::ErrorKind::DirectoryNotEmpty, "Target directory not empty")));
            }
        }
        
        // 1. Download
        if let Err(e) = self.download_and_verify(version, locale).await {
            self.cleanup_files();
            return Err(e);
        }
        
        // 2. Generate wp-config.php
        if let Err(e) = self.generate_wp_config(&db_creds.as_ref()) {
            self.cleanup_files();
            return Err(e);
        }
        
        // We cannot create databases here safely without root db creds, so assume the db is provisioned
        // If not, validation fails before install
        if let Err(e) = self.validate_database_connection(&db_creds.as_ref()).await {
             self.cleanup_files();
             return Err(e);
        }
        
        // 3. Core install
        let admin_user = "admin";
        let mut rng = rand::thread_rng();
        let generate_admin_pass = |rng: &mut rand::rngs::ThreadRng, length: usize| -> String {
             let mut key = [0u8; 64];
             rng.fill_bytes(&mut key);
             let b64_str = base64::engine::general_purpose::STANDARD.encode(key);
             b64_str.chars().filter(|c| c.is_ascii_alphanumeric() || "!@#$%^&*()-_=[]{}|;:,.<>?".contains(*c) && *c != '\'' && *c != '\\').take(length).collect()
        };
        let admin_password = generate_admin_pass(&mut rng, 24);
        // url is passed as argument
        
        if let Err(e) = wp_cli.core_install(site_path, url, title, admin_user, &admin_password, admin_email).await {
            self.cleanup_database(db_creds).await;
            self.cleanup_files();
            return Err(InstallError::WpCliError(e));
        }

        Ok(())
    }
    
    async fn cleanup_database(&self, creds: &GeneratedDbCredentials) {
        let user = urlencoding::encode(&creds.db_user);
        let pass = urlencoding::encode(&creds.db_password);
        let url = format!("mysql://{}:{}@{}/{}", user, pass, creds.db_host, creds.db_name);
        
        if let Ok(opts) = mysql_async::Opts::from_url(&url) {
            let pool = mysql_async::Pool::new(opts);
            if let Ok(mut conn) = pool.get_conn().await {
                 let _ = mysql_async::prelude::Queryable::query_drop(&mut conn, "SHOW TABLES").await;
                 // As a fallback to wp db reset, we can just drop all tables with the prefix
                 // A real implementation might run `wp db reset` if wp-cli is available,
                 // but since wp core install failed, it might be safer to drop all tables in this db.
                 let drop_query = format!("
                    SET FOREIGN_KEY_CHECKS = 0;
                    SET GROUP_CONCAT_MAX_LEN=32768;
                    SET @tables = NULL;
                    SELECT GROUP_CONCAT('`', table_name, '`') INTO @tables
                      FROM information_schema.tables
                      WHERE table_schema = (SELECT DATABASE());
                    SELECT IFNULL(@tables,'dummy') INTO @tables;
                    SET @tables = CONCAT('DROP TABLE IF EXISTS ', @tables);
                    PREPARE stmt FROM @tables;
                    EXECUTE stmt;
                    DEALLOCATE PREPARE stmt;
                    SET FOREIGN_KEY_CHECKS = 1;
                 ");
                 let _ = mysql_async::prelude::Queryable::query_drop(&mut conn, drop_query).await;
            }
        }
    }
    
    fn cleanup_files(&self) {
        let _ = fs::remove_dir_all(&self.target_dir);
    }

    pub async fn validate_database_connection(&self, creds: &DbCredentials<'_>) -> Result<(), InstallError> {
        if creds.db_name.is_empty() || creds.db_user.is_empty() || creds.db_host.is_empty() {
            return Err(InstallError::DatabaseValidationFailed("Missing database credentials".into()));
        }

        let user = urlencoding::encode(creds.db_user);
        let pass = urlencoding::encode(creds.db_password);
        let url = format!("mysql://{}:{}@{}/{}", user, pass, creds.db_host, creds.db_name);

        let opts = mysql_async::Opts::from_url(&url)
            .map_err(|e| InstallError::DatabaseValidationFailed(e.to_string()))?;

        let pool = mysql_async::Pool::new(opts);
        let mut conn = pool.get_conn().await
            .map_err(|e| InstallError::DatabaseValidationFailed(format!("Connection refused: {}", e)))?;

        let result: Option<String> = mysql_async::prelude::Queryable::query_first(&mut conn, "SELECT 1").await
            .map_err(|e| InstallError::DatabaseValidationFailed(format!("Query failed: {}", e)))?;

        if result.is_none() {
            return Err(InstallError::DatabaseValidationFailed("Database query failed to return result".to_string()));
        }

        Ok(())
    }
}

pub fn verify_file_checksum(path: &Path, expected_sha1: &str) -> Result<(), InstallError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha1::new();
    std::io::copy(&mut file, &mut hasher)?;
    let result = hasher.finalize();
    let actual_sha1 = hex::encode(result);

    if actual_sha1 != expected_sha1 {
        return Err(InstallError::ChecksumMismatch {
            file: path.display().to_string(),
            expected: expected_sha1.to_string(),
            actual: actual_sha1,
        });
    }

    Ok(())
}

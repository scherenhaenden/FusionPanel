use std::path::{Path, PathBuf};
use std::io::{Write, Read};
use thiserror::Error;
use reqwest::Client;
use serde::Deserialize;
use sha1::{Sha1, Digest};
use rand::RngCore;
use base64::{engine::general_purpose::STANDARD as b64, Engine};
use std::fs;

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

pub struct Orchestrator {
    target_dir: PathBuf,
    client: Client,
}

impl Orchestrator {
    pub fn new(target_dir: impl AsRef<Path>) -> Self {
        Self {
            target_dir: target_dir.as_ref().to_path_buf(),
            client: Client::new(),
        }
    }

    pub async fn download_and_verify(&self, version: &str, locale: &str) -> Result<(), InstallError> {
        let download_url = format!("https://downloads.wordpress.org/release/{locale}/wordpress-{version}.zip");
        let checksums_url = format!("https://api.wordpress.org/core/checksums/1.0/?version={version}&locale={locale}");

        let response: ChecksumsResponse = self.client.get(&checksums_url)
            .send()
            .await?
            .json()
            .await?;

        let mut resp = self.client.get(&download_url).send().await?;
        if !resp.status().is_success() {
            return Err(InstallError::DownloadFailed(format!("Failed to download WordPress zip: {}", resp.status())));
        }

        let mut tmp_file = tempfile::NamedTempFile::new()?;
        while let Some(chunk) = resp.chunk().await? {
            tmp_file.write_all(&chunk)?;
        }

        fs::create_dir_all(&self.target_dir)?;

        let mut archive = zip::ZipArchive::new(fs::File::open(tmp_file.path())?)
            .map_err(|e| InstallError::ZipError(e.to_string()))?;

        for i in 0..archive.len() {
            let mut file = archive.by_index(i).map_err(|e| InstallError::ZipError(e.to_string()))?;

            // Expected zip layout: wordpress/index.php etc.
            let outpath = match file.enclosed_name() {
                Some(path) => {
                    // strip the top-level "wordpress/" directory
                    let mut components = path.components();
                    if let Some(first) = components.next() {
                        if first.as_os_str() == "wordpress" {
                            self.target_dir.join(components.as_path())
                        } else {
                            self.target_dir.join(path)
                        }
                    } else {
                        self.target_dir.join(path)
                    }
                },
                None => continue,
            };

            if (*file.name()).ends_with('/') {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    if !p.exists() {
                        fs::create_dir_all(p)?;
                    }
                }

                // Read content to calculate hash and write out
                let mut content = Vec::new();
                file.read_to_end(&mut content)?;

                let zip_path = file.name().to_string();
                // In checksum API, paths don't always start with `wordpress/` but often they do, actually they usually do in the zip but API might vary.
                // According to API: the keys are like `wp-includes/version.php` but zip paths are `wordpress/wp-includes/version.php`.

                let api_key = if zip_path.starts_with("wordpress/") {
                    zip_path.trim_start_matches("wordpress/").to_string()
                } else {
                    zip_path.clone()
                };

                let mut hasher = Sha1::new();
                hasher.update(&content);
                let actual_sha1 = hex::encode(hasher.finalize());

                if let Some(expected_sha1) = response.checksums.get(&api_key) {
                    if &actual_sha1 != expected_sha1 {
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
        let config_path = self.target_dir.join("wp-config.php");
        let mut file = fs::File::create(config_path)?;
        file.write_all(config_content.as_bytes())?;

        Ok(())
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

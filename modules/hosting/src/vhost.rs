#![forbid(unsafe_code)]

use crate::domain::error::{DomainError, Result};
use crate::domain::{HostingSpec, SiteConfig};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Trait to allow mocking filesystem and system command operations in unit tests.
pub trait SystemEnvironment: Send + Sync {
    fn write_file(&self, path: &Path, content: &str) -> Result<()>;
    fn remove_file(&self, path: &Path) -> Result<()>;
    fn symlink(&self, src: &Path, dst: &Path) -> Result<()>;
    fn read_link(&self, path: &Path) -> Result<PathBuf>;
    fn file_exists(&self, path: &Path) -> bool;
    fn test_nginx_config(&self) -> Result<()>;
    fn reload_nginx(&self) -> Result<()>;
    fn test_php_fpm_config(&self, version: &str) -> Result<()>;
    fn reload_php_fpm(&self, version: &str) -> Result<()>;
}

pub struct RealSystemEnvironment;

impl SystemEnvironment for RealSystemEnvironment {
    fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                DomainError::OperationFailed(format!("Failed to create dir: {}", e))
            })?;
        }
        std::fs::write(path, content).map_err(|e| {
            DomainError::OperationFailed(format!("Failed to write file {:?}: {}", path, e))
        })
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        std::fs::remove_file(path).map_err(|e| {
            DomainError::OperationFailed(format!("Failed to remove file {:?}: {}", path, e))
        })
    }

    fn symlink(&self, src: &Path, dst: &Path) -> Result<()> {
        std::os::unix::fs::symlink(src, dst).map_err(|e| {
            DomainError::OperationFailed(format!("Failed to symlink {:?} to {:?}: {}", src, dst, e))
        })
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf> {
        std::fs::read_link(path).map_err(|e| {
            DomainError::OperationFailed(format!("Failed to read link {:?}: {}", path, e))
        })
    }

    fn file_exists(&self, path: &Path) -> bool {
        path.exists()
    }

    fn test_nginx_config(&self) -> Result<()> {
        let status = Command::new("nginx").arg("-t").status().map_err(|e| {
            DomainError::OperationFailed(format!("Failed to execute nginx -t: {}", e))
        })?;

        if status.success() {
            Ok(())
        } else {
            Err(DomainError::OperationFailed(
                "Nginx configuration test failed".into(),
            ))
        }
    }

    fn reload_nginx(&self) -> Result<()> {
        let status = Command::new("nginx")
            .arg("-s")
            .arg("reload")
            .status()
            .map_err(|e| {
                DomainError::OperationFailed(format!("Failed to execute nginx -s reload: {}", e))
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(DomainError::OperationFailed("Nginx reload failed".into()))
        }
    }

    fn test_php_fpm_config(&self, version: &str) -> Result<()> {
        let status = Command::new(format!("php-fpm{}", version))
            .arg("-t")
            .status()
            .map_err(|e| {
                DomainError::OperationFailed(format!(
                    "Failed to execute php-fpm{} -t: {}",
                    version, e
                ))
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(DomainError::OperationFailed(
                "PHP-FPM configuration test failed".into(),
            ))
        }
    }

    fn reload_php_fpm(&self, version: &str) -> Result<()> {
        let status = Command::new("systemctl")
            .arg("reload")
            .arg(format!("php{}-fpm", version))
            .status()
            .map_err(|e| {
                DomainError::OperationFailed(format!("Failed to reload php-fpm{}: {}", version, e))
            })?;

        if status.success() {
            Ok(())
        } else {
            Err(DomainError::OperationFailed("PHP-FPM reload failed".into()))
        }
    }
}

pub struct VhostOrchestrator {
    env: Box<dyn SystemEnvironment>,
    nginx_sites_available: PathBuf,
    nginx_sites_enabled: PathBuf,
    php_fpm_pool_base: PathBuf, // e.g., /etc/php
}

impl VhostOrchestrator {
    pub fn new(
        env: Box<dyn SystemEnvironment>,
        nginx_sites_available: PathBuf,
        nginx_sites_enabled: PathBuf,
        php_fpm_pool_base: PathBuf,
    ) -> Self {
        Self {
            env,
            nginx_sites_available,
            nginx_sites_enabled,
            php_fpm_pool_base,
        }
    }

    /// Generate and apply configuration, ensuring atomic reload
    pub fn apply_site(&self, site: &SiteConfig) -> Result<()> {
        let domain = site.domain.as_str();
        let nginx_conf_name = format!("{}.conf", domain);
        let available_path = self.nginx_sites_available.join(&nginx_conf_name);
        let enabled_path = self.nginx_sites_enabled.join(&nginx_conf_name);

        // 1. Generate configs
        let nginx_config_content = crate::config::nginx::generate_nginx_config(site)?;

        let mut php_info = None;
        if let HostingSpec::Php(php_spec) = &site.spec {
            let php_config_content = crate::config::php_fpm::generate_php_fpm_config(site)?;
            let version = php_spec.version.as_str();
            // e.g. /etc/php/8.2/fpm/pool.d
            let pool_dir = self
                .php_fpm_pool_base
                .join(version)
                .join("fpm")
                .join("pool.d");
            let pool_path = pool_dir.join(format!("{}.conf", php_spec.pool_name.as_str()));
            php_info = Some((version.to_string(), pool_path, php_config_content));
        }

        // 2. Write configs to disk
        self.env
            .write_file(&available_path, &nginx_config_content)?;

        if let Some((_, ref pool_path, ref content)) = php_info {
            self.env.write_file(pool_path, content)?;
        }

        // 3. Enable site (symlink)
        // If symlink already exists, remove it first to recreate correctly
        if self.env.file_exists(&enabled_path) {
            let _ = self.env.remove_file(&enabled_path);
        }
        self.env.symlink(&available_path, &enabled_path)?;

        // 4. Test configurations
        let nginx_test_result = self.env.test_nginx_config();

        let php_test_result = if let Some((ref version, _, _)) = php_info {
            self.env.test_php_fpm_config(version)
        } else {
            Ok(())
        };

        // 5. Atomic reload / Rollback
        if nginx_test_result.is_err() || php_test_result.is_err() {
            // Rollback: remove the symlink to disable the broken site
            let _ = self.env.remove_file(&enabled_path);

            // Optionally remove the written files
            let _ = self.env.remove_file(&available_path);
            if let Some((_, ref pool_path, _)) = php_info {
                let _ = self.env.remove_file(pool_path);
            }

            return Err(DomainError::OperationFailed(
                "Configuration test failed, rolled back site config".into(),
            ));
        }

        // Tests passed, reload services
        self.env.reload_nginx()?;

        if let Some((ref version, _, _)) = php_info {
            self.env.reload_php_fpm(version)?;
        }

        Ok(())
    }

    pub fn disable_site(&self, site: &SiteConfig) -> Result<()> {
        let domain = site.domain.as_str();
        let nginx_conf_name = format!("{}.conf", domain);
        let enabled_path = self.nginx_sites_enabled.join(&nginx_conf_name);

        if self.env.file_exists(&enabled_path) {
            self.env.remove_file(&enabled_path)?;

            // Validate before reloading after removal
            if self.env.test_nginx_config().is_ok() {
                self.env.reload_nginx()?;
            } else {
                return Err(DomainError::OperationFailed(
                    "Nginx test failed after disabling site".into(),
                ));
            }
        }

        Ok(())
    }

    pub fn delete_site(&self, site: &SiteConfig) -> Result<()> {
        self.disable_site(site)?;

        let domain = site.domain.as_str();
        let nginx_conf_name = format!("{}.conf", domain);
        let available_path = self.nginx_sites_available.join(&nginx_conf_name);

        if self.env.file_exists(&available_path) {
            self.env.remove_file(&available_path)?;
        }

        if let HostingSpec::Php(php_spec) = &site.spec {
            let version = php_spec.version.as_str();
            let pool_dir = self
                .php_fpm_pool_base
                .join(version)
                .join("fpm")
                .join("pool.d");
            let pool_path = pool_dir.join(format!("{}.conf", php_spec.pool_name.as_str()));

            if self.env.file_exists(&pool_path) {
                self.env.remove_file(&pool_path)?;
                if self.env.test_php_fpm_config(version).is_ok() {
                    self.env.reload_php_fpm(version)?;
                }
            }
        }

        Ok(())
    }
}

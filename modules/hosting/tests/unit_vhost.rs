#![forbid(unsafe_code)]

use hosting::domain::{
    DomainName, HostingSpec, PhpConfig, SiteConfig, SiteId, StaticConfig, TenantId,
};

use hosting::domain::error::{DomainError, Result};
use hosting::domain::identity::ResourceOwner;
use hosting::vhost::{SystemEnvironment, VhostOrchestrator};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[derive(Clone, Default)]
struct MockEnvironment {
    files: Arc<Mutex<HashMap<PathBuf, String>>>,
    symlinks: Arc<Mutex<HashMap<PathBuf, PathBuf>>>,
    removed_files: Arc<Mutex<Vec<PathBuf>>>,
    nginx_test_fails: bool,
    php_test_fails: bool,
    reload_nginx_called: Arc<Mutex<bool>>,
    reload_php_fpm_called: Arc<Mutex<bool>>,
}

impl SystemEnvironment for MockEnvironment {
    fn write_file(&self, path: &Path, content: &str) -> Result<()> {
        self.files
            .lock()
            .unwrap()
            .insert(path.to_path_buf(), content.to_string());
        Ok(())
    }

    fn remove_file(&self, path: &Path) -> Result<()> {
        self.files.lock().unwrap().remove(path);
        self.symlinks.lock().unwrap().remove(path);
        self.removed_files.lock().unwrap().push(path.to_path_buf());
        Ok(())
    }

    fn symlink(&self, src: &Path, dst: &Path) -> Result<()> {
        self.symlinks
            .lock()
            .unwrap()
            .insert(dst.to_path_buf(), src.to_path_buf());
        Ok(())
    }

    fn read_link(&self, path: &Path) -> Result<PathBuf> {
        self.symlinks
            .lock()
            .unwrap()
            .get(path)
            .cloned()
            .ok_or_else(|| DomainError::OperationFailed("Link not found".into()))
    }

    fn file_exists(&self, path: &Path) -> bool {
        self.files.lock().unwrap().contains_key(path)
            || self.symlinks.lock().unwrap().contains_key(path)
    }

    fn test_nginx_config(&self) -> Result<()> {
        if self.nginx_test_fails {
            Err(DomainError::OperationFailed("Nginx test failed".into()))
        } else {
            Ok(())
        }
    }

    fn reload_nginx(&self) -> Result<()> {
        *self.reload_nginx_called.lock().unwrap() = true;
        Ok(())
    }

    fn test_php_fpm_config(&self, _version: &str) -> Result<()> {
        if self.php_test_fails {
            Err(DomainError::OperationFailed("PHP test failed".into()))
        } else {
            Ok(())
        }
    }

    fn reload_php_fpm(&self, _version: &str) -> Result<()> {
        *self.reload_php_fpm_called.lock().unwrap() = true;
        Ok(())
    }
}

fn create_static_site() -> SiteConfig {
    SiteConfig {
        id: SiteId(uuid::Uuid::new_v4()),
        tenant_id: TenantId(uuid::Uuid::new_v4()),
        domain: DomainName::new("example.com").unwrap(),
        document_root: "/srv/fusionpanel/sites/example.com"
            .to_string()
            .try_into()
            .unwrap(),
        spec: HostingSpec::Static(StaticConfig),
        port: 8080.try_into().unwrap(),
        owner: ResourceOwner::ExternallyManaged,
        run_as_user: "tenant-1".to_string().try_into().unwrap(),
        external_bindings: vec![],
    }
}

fn create_php_site() -> SiteConfig {
    SiteConfig {
        id: SiteId(uuid::Uuid::new_v4()),
        tenant_id: TenantId(uuid::Uuid::new_v4()),
        domain: DomainName::new("php.example.com").unwrap(),
        document_root: "/srv/fusionpanel/sites/php.example.com"
            .to_string()
            .try_into()
            .unwrap(),
        spec: HostingSpec::Php(PhpConfig {
            version: "8.2".to_string().try_into().unwrap(),
            pool_name: "php_example_com".to_string().try_into().unwrap(),
            memory_limit: "256M".to_string().try_into().unwrap(),
        }),
        port: 8080.try_into().unwrap(),
        owner: ResourceOwner::ExternallyManaged,
        run_as_user: "tenant-2".to_string().try_into().unwrap(),
        external_bindings: vec![],
    }
}

#[test]
fn test_apply_static_site_success() {
    let env = MockEnvironment::default();
    let orchestrator = VhostOrchestrator::new(
        Box::new(env.clone()),
        PathBuf::from("/etc/nginx/sites-available"),
        PathBuf::from("/etc/nginx/sites-enabled"),
        PathBuf::from("/etc/php"),
    );

    let site = create_static_site();
    let result = orchestrator.apply_site(&site);

    assert!(result.is_ok());

    let files = env.files.lock().unwrap();
    let available_path = PathBuf::from("/etc/nginx/sites-available/example.com.conf");
    assert!(files.contains_key(&available_path));

    let symlinks = env.symlinks.lock().unwrap();
    let enabled_path = PathBuf::from("/etc/nginx/sites-enabled/example.com.conf");
    assert_eq!(symlinks.get(&enabled_path), Some(&available_path));

    assert!(*env.reload_nginx_called.lock().unwrap());
}

#[test]
fn test_apply_php_site_success() {
    let env = MockEnvironment::default();
    let orchestrator = VhostOrchestrator::new(
        Box::new(env.clone()),
        PathBuf::from("/etc/nginx/sites-available"),
        PathBuf::from("/etc/nginx/sites-enabled"),
        PathBuf::from("/etc/php"),
    );

    let site = create_php_site();
    let result = orchestrator.apply_site(&site);

    assert!(result.is_ok());

    let files = env.files.lock().unwrap();
    let available_path = PathBuf::from("/etc/nginx/sites-available/php.example.com.conf");
    let pool_path = PathBuf::from("/etc/php/8.2/fpm/pool.d/php_example_com.conf");

    assert!(files.contains_key(&available_path));
    assert!(files.contains_key(&pool_path));

    let symlinks = env.symlinks.lock().unwrap();
    let enabled_path = PathBuf::from("/etc/nginx/sites-enabled/php.example.com.conf");
    assert_eq!(symlinks.get(&enabled_path), Some(&available_path));

    assert!(*env.reload_nginx_called.lock().unwrap());
    assert!(*env.reload_php_fpm_called.lock().unwrap());
}

#[test]
fn test_apply_site_rollback_on_nginx_failure() {
    let mut env = MockEnvironment::default();
    env.nginx_test_fails = true;
    let orchestrator = VhostOrchestrator::new(
        Box::new(env.clone()),
        PathBuf::from("/etc/nginx/sites-available"),
        PathBuf::from("/etc/nginx/sites-enabled"),
        PathBuf::from("/etc/php"),
    );

    let site = create_static_site();
    let result = orchestrator.apply_site(&site);

    assert!(result.is_err());

    // Check that reload was not called
    assert!(!*env.reload_nginx_called.lock().unwrap());

    // Check that rollback occurred (symlink removed)
    let symlinks = env.symlinks.lock().unwrap();
    let enabled_path = PathBuf::from("/etc/nginx/sites-enabled/example.com.conf");
    assert!(!symlinks.contains_key(&enabled_path));
}

#[test]
fn test_delete_php_site() {
    let env = MockEnvironment::default();
    let orchestrator = VhostOrchestrator::new(
        Box::new(env.clone()),
        PathBuf::from("/etc/nginx/sites-available"),
        PathBuf::from("/etc/nginx/sites-enabled"),
        PathBuf::from("/etc/php"),
    );

    let site = create_php_site();

    // Setup initial state
    let available_path = PathBuf::from("/etc/nginx/sites-available/php.example.com.conf");
    let enabled_path = PathBuf::from("/etc/nginx/sites-enabled/php.example.com.conf");
    let pool_path = PathBuf::from("/etc/php/8.2/fpm/pool.d/php_example_com.conf");

    env.files
        .lock()
        .unwrap()
        .insert(available_path.clone(), "".into());
    env.files
        .lock()
        .unwrap()
        .insert(pool_path.clone(), "".into());
    env.symlinks
        .lock()
        .unwrap()
        .insert(enabled_path.clone(), available_path.clone());

    let result = orchestrator.delete_site(&site);

    assert!(result.is_ok());

    let files = env.files.lock().unwrap();
    let symlinks = env.symlinks.lock().unwrap();

    assert!(!symlinks.contains_key(&enabled_path));
    assert!(!files.contains_key(&available_path));
    assert!(!files.contains_key(&pool_path));

    assert!(*env.reload_nginx_called.lock().unwrap());
    assert!(*env.reload_php_fpm_called.lock().unwrap());
}

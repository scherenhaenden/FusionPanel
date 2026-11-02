use std::convert::TryInto;
use std::process::Command;
use uuid::Uuid;

use hosting::config::{generate_nginx_config, generate_php_fpm_config};
use hosting::domain::validation::{MemoryLimit, PhpVersion, PoolName, UpstreamUrl};
use hosting::domain::{DomainName, HostingSpec, PhpConfig, SiteConfig, SiteId, TenantId};

#[test]
fn test_generate_and_validate_configs() {
    let site_id = SiteId(Uuid::new_v4());
    let tenant_id = TenantId(Uuid::new_v4());

    let domain = DomainName::new("example.com").unwrap();

    let spec = HostingSpec::Php(PhpConfig {
        version: "8.2".to_string().try_into().unwrap(),
        pool_name: "example_pool".to_string().try_into().unwrap(),
        memory_limit: "256M".to_string().try_into().unwrap(),
    });

    let site = SiteConfig::new(
        site_id,
        tenant_id,
        domain,
        "/srv/fusionpanel/sites/example.com".into(),
        spec,
    )
    .unwrap();

    let nginx_config = generate_nginx_config(&site).unwrap();
    let php_config = generate_php_fpm_config(&site).unwrap();

    // Check docker availability first.
    let docker_check = Command::new("docker")
        .arg("run")
        .arg("--rm")
        .arg("alpine")
        .arg("echo")
        .arg("hello")
        .output();

    if let Ok(out) = docker_check {
        if out.status.success() {
            // Docker works normally, proceed with syntax checks using local temporary files and bind mounts
            let temp_dir = tempfile::tempdir().unwrap();

            // NGINX config test
            let nginx_conf_path = temp_dir.path().join("test_nginx.conf");
            std::fs::write(&nginx_conf_path, &nginx_config).unwrap();
            let nginx_out = Command::new("docker")
                .arg("run")
                .arg("--rm")
                .arg("-v")
                .arg(format!(
                    "{}:/etc/nginx/conf.d/test.conf:ro",
                    nginx_conf_path.display()
                ))
                .arg("nginx:alpine")
                .arg("nginx")
                .arg("-t")
                .output()
                .unwrap();
            assert!(
                nginx_out.status.success(),
                "Nginx syntax check failed. Stdout: {}, Stderr: {}",
                String::from_utf8_lossy(&nginx_out.stdout),
                String::from_utf8_lossy(&nginx_out.stderr)
            );

            // PHP-FPM config test
            let php_conf_path = temp_dir.path().join("test_php.conf");
            std::fs::write(&php_conf_path, &php_config).unwrap();
            let php_out = Command::new("docker")
                .arg("run")
                .arg("--rm")
                .arg("-v")
                .arg(format!(
                    "{}:/usr/local/etc/php-fpm.d/test.conf:ro",
                    php_conf_path.display()
                ))
                .arg("php:fpm-alpine")
                .arg("php-fpm")
                .arg("-t")
                .output()
                .unwrap();
            assert!(
                php_out.status.success(),
                "PHP-FPM syntax check failed. Stdout: {}, Stderr: {}",
                String::from_utf8_lossy(&php_out.stdout),
                String::from_utf8_lossy(&php_out.stderr)
            );
        } else {
            panic!("BLOCKED: Docker is present but fails to run basic containers.");
        }
    } else {
        panic!("BLOCKED: Docker not available, skipping actual syntax verification.");
    }
}

#[test]
fn test_validation_rejects_injections() {
    let site_id = SiteId(Uuid::new_v4());
    let tenant_id = TenantId(Uuid::new_v4());
    let domain = DomainName::new("example.com").unwrap();

    // Test Document Root traversal/null byte injection
    let doc_res = SiteConfig::new(
        site_id.clone(),
        tenant_id.clone(),
        domain.clone(),
        "/srv/fusionpanel/sites/../etc/passwd\0".to_string(),
        HostingSpec::Static(hosting::domain::StaticConfig),
    );
    assert!(doc_res.is_err(), "Should reject invalid document root");

    // Test PHP version invalid characters
    let bad_php: Result<PhpVersion, _> = "8.2; rm -rf /".to_string().try_into();
    assert!(bad_php.is_err(), "Should reject invalid PHP version");

    // Test Pool Name invalid characters
    let bad_pool: Result<PoolName, _> = "pool] \n user=root".to_string().try_into();
    assert!(bad_pool.is_err(), "Should reject invalid pool name");

    // Test Memory Limit invalid characters
    let bad_memory: Result<MemoryLimit, _> = "256M; user=root".to_string().try_into();
    assert!(bad_memory.is_err(), "Should reject invalid memory limit");

    // Test Upstream URL invalid characters
    let bad_upstream: Result<UpstreamUrl, _> = "http://localhost \n proxy_pass http://evil.com;"
        .to_string()
        .try_into();
    assert!(bad_upstream.is_err(), "Should reject invalid upstream URL");
}

#![forbid(unsafe_code)]

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
        8080,
        hosting::domain::ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test-user".to_string().try_into().unwrap(),
        vec![],
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
                .arg("sh")
                .arg("-c")
                .arg(format!("adduser -D {} && php-fpm -t", site.run_as_user.as_str()))
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
        8080,
        hosting::domain::ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test-user".to_string().try_into().unwrap(),
        vec![],
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

    // Test privileged port conflict/detection
    let port_res = SiteConfig::new(
        site_id.clone(),
        tenant_id.clone(),
        domain.clone(),
        "/srv/fusionpanel/sites/example.com".to_string(),
        HostingSpec::Static(hosting::domain::StaticConfig),
        80, // Privileged port
        hosting::domain::ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test-user".to_string().try_into().unwrap(),
        vec![], // < 1024 should be rejected
    );
    assert!(port_res.is_err(), "Should reject privileged ports");
}

#[test]
fn test_vhost_collision_and_socket_permissions() {
    let site_id = SiteId(Uuid::new_v4());
    let tenant_id = TenantId(Uuid::new_v4());
    let domain = DomainName::new("example.com").unwrap();
    let spec = HostingSpec::Php(PhpConfig {
        version: "8.2".to_string().try_into().unwrap(),
        pool_name: "example_pool".to_string().try_into().unwrap(),
        memory_limit: "256M".to_string().try_into().unwrap(),
    });

    let site1 = SiteConfig::new(
        site_id.clone(),
        tenant_id.clone(),
        domain.clone(),
        "/srv/fusionpanel/sites/example.com".to_string(),
        spec.clone(),
        8080,
        hosting::domain::ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test-user".to_string().try_into().unwrap(),
        vec![],
    )
    .unwrap();

    let nginx_config = hosting::config::generate_nginx_config(&site1).unwrap();
    let php_fpm_config = hosting::config::generate_php_fpm_config(&site1).unwrap();

    // Check vhost collision setup
    let site2 = SiteConfig::new(
        SiteId(Uuid::new_v4()),
        tenant_id.clone(),
        domain.clone(), // Same domain
        "/srv/fusionpanel/sites/example.com".to_string(),
        spec.clone(),
        8080,
        hosting::domain::ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test-user".to_string().try_into().unwrap(),
        vec![], // Same port
    )
    .unwrap();

    let mut registry = hosting::domain::HostingRegistry::new();
    assert!(registry.register(site1.clone()).is_ok());

    // Should return conflict for same domain + same port
    let collision_err = registry.register(site2);
    assert!(collision_err.is_err());

    // Check port allocation conflict
    let site3 = SiteConfig::new(
        SiteId(Uuid::new_v4()),
        tenant_id.clone(),
        DomainName::new("example.org").unwrap(), // Different domain
        "/srv/fusionpanel/sites/example.org".to_string(),
        spec.clone(),
        8080,
        hosting::domain::ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test-user".to_string().try_into().unwrap(),
        vec![], // Same port
    )
    .unwrap();
    let port_conflict_err = registry.register(site3);
    assert!(port_conflict_err.is_err());

    // In our test, we just check that the config generated contains the right properties
    assert!(nginx_config.contains("listen 8080;"));
    assert!(nginx_config.contains("server_name example.com;"));

    // Check unix socket permissions
    assert!(php_fpm_config.contains("listen.mode = 0660"));
    assert!(php_fpm_config.contains("listen.owner = test-user"));
    assert!(php_fpm_config.contains("listen.group = test-user"));
}

#[test]
fn test_external_binding_validation() {
    use hosting::domain::{TenantId, ExternalBinding};
    use hosting::domain::validation::BindingReadWritePolicy;
    use hosting::domain::ResourceOwner;
    use uuid::Uuid;

    let tenant_id = TenantId(Uuid::new_v4());

    // Test Path Traversal
    let bad_binding = ExternalBinding::new(
        "/src/../secret",
        "/canon",
        "/target",
        ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test",
        BindingReadWritePolicy::ReadOnly,
        "REQ-123"
    );
    assert!(bad_binding.is_err(), "Should reject traversal in bindings");

    // Test Protected Path
    let bad_binding2 = ExternalBinding::new(
        "/src",
        "/etc/passwd",
        "/target",
        ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test",
        BindingReadWritePolicy::ReadOnly,
        "REQ-123"
    );
    assert!(bad_binding2.is_err(), "Should reject protected paths");

    // Test Missing Review Ref
    let bad_binding3 = ExternalBinding::new(
        "/src",
        "/canon",
        "/target",
        ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test",
        BindingReadWritePolicy::ReadOnly,
        ""
    );
    assert!(bad_binding3.is_err(), "Should require review reference");

    // Valid binding
    let good_binding = ExternalBinding::new(
        "/src",
        "/canon",
        "/target",
        ResourceOwner::PanelOwned { tenant_id: tenant_id.clone() },
        "test",
        BindingReadWritePolicy::ReadOnly,
        "REQ-123"
    ).unwrap();

    assert_eq!(good_binding.source_path, "/src");
}

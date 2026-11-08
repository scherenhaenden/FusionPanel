#![forbid(unsafe_code)]

use hosting::domain::identity::{ResourceOwner, SiteId, TenantId};
use hosting::domain::validation::{DomainName, MemoryLimit, PhpVersion, PoolName};
use hosting::domain::{HostingSpec, PhpConfig, SiteConfig, StaticConfig};
use std::convert::TryInto;
use uuid::Uuid;

fn create_mock_site(is_php: bool) -> SiteConfig {
    let spec = if is_php {
        let version: PhpVersion = "8.1".to_string().try_into().unwrap();
        let pool_name: PoolName = "test_pool".to_string().try_into().unwrap();
        let memory_limit: MemoryLimit = "128M".to_string().try_into().unwrap();
        HostingSpec::Php(PhpConfig {
            version,
            pool_name,
            memory_limit,
        })
    } else {
        HostingSpec::Static(StaticConfig)
    };

    let site_id = SiteId(Uuid::nil());
    let tenant_id = TenantId(Uuid::nil());

    SiteConfig::new(
        site_id,
        tenant_id,
        DomainName::new("example.com").unwrap(),
        "/srv/fusionpanel/sites/example.com".to_string(),
        spec,
        8080,
        ResourceOwner::PanelOwned { tenant_id },
        "test-user".to_string().try_into().unwrap(),
        vec![],
    )
    .unwrap()
}

#[test]
fn test_generate_nginx_php_fpm_config_valid() {
    let site = create_mock_site(true);
    let site_id = site.id.0;

    let config = hosting::nginx::php_fpm::generate_nginx_php_fpm_config(&site)
        .expect("Config generation failed");

    // Check fastcgi_split_path_info
    assert!(config.contains("fastcgi_split_path_info ^(.+\\.php)(/.+)$;"));

    // Check SCRIPT_FILENAME
    assert!(config.contains("fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;"));

    // Check secure upload limits
    assert!(config.contains("client_max_body_size"));

    // Check socket path isolation
    assert!(config.contains(&format!(
        "fastcgi_pass unix:/run/php/php-fpm-{}.sock;",
        site_id
    )));
}

#[test]
fn test_generate_nginx_php_fpm_config_invalid_spec() {
    let site = create_mock_site(false);
    let result = hosting::nginx::php_fpm::generate_nginx_php_fpm_config(&site);

    assert!(result.is_err());
    let err = result.unwrap_err();
    match err {
        hosting::domain::error::DomainError::InvalidInput(msg) => {
            assert_eq!(msg, "Site is not configured for PHP");
        }
        _ => panic!("Unexpected error type"),
    }
}

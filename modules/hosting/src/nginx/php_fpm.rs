#![forbid(unsafe_code)]

use hosting::domain::{error::Result, SiteConfig, HostingSpec};

pub fn generate_nginx_php_fpm_config(site: &SiteConfig) -> Result<String> {
    match &site.spec {
        HostingSpec::Php(_) => {
            let site_id = &site.id.0;
            let socket_path = format!("/run/php/php-fpm-{}.sock", site_id);

            let config = format!(
                "    location ~ \\.php$ {{\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20fastcgi_split_path_info ^(.+\\.php)(/.+)$;\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20include fastcgi_params;\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20fastcgi_pass unix:{};\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20client_max_body_size 50M;\n\
                 \x20\x20\x20\x20}}\n",
                socket_path
            );
            Ok(config)
        }
        _ => {
            Err(hosting::domain::error::DomainError::InvalidInput(
                "Site is not configured for PHP".into(),
            ))
        }
    }
}

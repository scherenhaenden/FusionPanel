use crate::domain::{SiteConfig, HostingSpec, error::Result};

pub fn generate_nginx_config(site: &SiteConfig) -> Result<String> {
    let domain = site.domain.as_str();
    let root = &site.document_root;
    let site_id = &site.id.0;
    let tenant_id = &site.tenant_id.0;

    let mut config = format!(
        "# Nginx configuration for {}\n\
         # Site ID: {}\n\
         # Tenant ID: {}\n\
         server {{\n\
         \x20\x20\x20\x20listen 80;\n\
         \x20\x20\x20\x20server_name {};\n\
         \x20\x20\x20\x20root {};\n\
         \x20\x20\x20\x20index index.php index.html index.htm;\n\n",
        domain, site_id, tenant_id, domain, root
    );

    match &site.spec {
        HostingSpec::Static(_) => {
            config.push_str(
                "\x20\x20\x20\x20location / {\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20try_files $uri $uri/ =404;\n\
                 \x20\x20\x20\x20}\n"
            );
        },
        HostingSpec::Php(_php) => {
            // Need a valid PHP-FPM upstream here. We'll use a unix socket for the site.
            let socket_path = format!("/run/php/php-fpm-{}.sock", site_id);
            config.push_str(&format!(
                "\x20\x20\x20\x20location / {{\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20try_files $uri $uri/ /index.php?$query_string;\n\
                 \x20\x20\x20\x20}}\n\n\
                 \x20\x20\x20\x20location ~ \\.php$ {{\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20include fastcgi_params;\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20fastcgi_pass unix:{};\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20fastcgi_param SCRIPT_FILENAME $document_root$fastcgi_script_name;\n\
                 \x20\x20\x20\x20}}\n", socket_path
            ));
        },
        HostingSpec::ReverseProxy(rp) => {
            config.push_str(&format!(
                "\x20\x20\x20\x20location / {{\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20proxy_pass {};\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20proxy_set_header Host $host;\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20proxy_set_header X-Real-IP $remote_addr;\n\
                 \x20\x20\x20\x20}}\n", rp.upstream_url
            ));
        }
    }

    config.push_str("}\n");

    Ok(config)
}

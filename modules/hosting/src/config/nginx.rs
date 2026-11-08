use crate::domain::{error::Result, HostingSpec, SiteConfig, UpstreamTlsTrust};

pub fn generate_nginx_config(site: &SiteConfig) -> Result<String> {
    let domain = site.domain.as_str();
    let root = site.document_root.as_str();
    let site_id = &site.id.0;
    let tenant_id = &site.tenant_id.0;

    let port = site.port.as_u16();
    let mut config = format!(
        "# Nginx configuration for {}\n\
         # Site ID: {}\n\
         # Tenant ID: {}\n\
         server {{\n\
         \x20\x20\x20\x20listen {};\n\
         \x20\x20\x20\x20server_name {};\n\
         \x20\x20\x20\x20root {};\n\
         \x20\x20\x20\x20index index.php index.html index.htm;\n\n",
        domain, site_id, tenant_id, port, domain, root
    );

    match &site.spec {
        HostingSpec::Static(_) => {
            config.push_str(
                "\x20\x20\x20\x20location / {\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20try_files $uri $uri/ =404;\n\
                 \x20\x20\x20\x20}\n",
            );
        }
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
        }
        HostingSpec::ReverseProxy(rp) => {
            let mut proxy_config = format!(
                "\x20\x20\x20\x20location / {{\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20proxy_pass {};\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20proxy_set_header Host $host;\n\
                 \x20\x20\x20\x20\x20\x20\x20\x20proxy_set_header X-Real-IP $remote_addr;\n",
                rp.upstream_url.as_str()
            );

            if let Some(tls) = &rp.tls_policy {
                match &tls.trust {
                    UpstreamTlsTrust::SystemCa => {
                        proxy_config.push_str("        proxy_ssl_verify on;\n");
                        proxy_config.push_str("        proxy_ssl_trusted_certificate /etc/ssl/certs/ca-certificates.crt;\n");
                    }
                    UpstreamTlsTrust::InternalCa { ca_cert_path } => {
                        proxy_config.push_str("        proxy_ssl_verify on;\n");
                        proxy_config.push_str(&format!(
                            "        proxy_ssl_trusted_certificate {};\n",
                            ca_cert_path
                        ));
                    }
                    UpstreamTlsTrust::InsecureCompatibility { audit_reason } => {
                        proxy_config.push_str("        proxy_ssl_verify off;\n");
                        proxy_config.push_str(&format!(
                            "        # Insecure compatibility enabled. Reason: {}\n",
                            audit_reason
                        ));
                    }
                }

                if let Some(expected_name) = &tls.expected_server_name {
                    proxy_config.push_str("        proxy_ssl_server_name on;\n");
                    proxy_config.push_str(&format!(
                        "        proxy_ssl_name {};\n",
                        expected_name.as_str()
                    ));
                } else {
                    proxy_config.push_str("        proxy_ssl_server_name off;\n");
                }
            }

            proxy_config.push_str("    }\n");
            config.push_str(&proxy_config);
        }
    }

    config.push_str("}\n");

    Ok(config)
}

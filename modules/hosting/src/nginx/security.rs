#![forbid(unsafe_code)]

pub fn generate_http_redirect(domain: &str) -> String {
    format!(
        "server {{\n\
        \x20\x20\x20\x20listen 80;\n\
        \x20\x20\x20\x20listen [::]:80;\n\
        \x20\x20\x20\x20server_name {};\n\
        \x20\x20\x20\x20return 301 https://$host$request_uri;\n\
        }}\n",
        domain
    )
}

pub fn generate_tls_config() -> String {
    "ssl_protocols TLSv1.2 TLSv1.3;\n\
ssl_ciphers HIGH:!aNULL:!MD5;\n\
ssl_prefer_server_ciphers off;\n\
add_header Strict-Transport-Security \"max-age=31536000; includeSubDomains\" always;\n"
        .to_string()
}

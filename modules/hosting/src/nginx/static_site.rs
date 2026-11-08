#![forbid(unsafe_code)]

pub fn escape_nginx_string(input: &str) -> String {
    let mut escaped = String::with_capacity(input.len());
    for c in input.chars() {
        match c {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '$' => escaped.push_str("\\$"),
            _ => escaped.push(c),
        }
    }
    escaped
}

pub fn generate_static_site_config(domain: &str, document_root: &str, port: u16) -> String {
    let escaped_domain = escape_nginx_string(domain);
    let escaped_root = escape_nginx_string(document_root);

    format!(
        r#"server {{
    listen {port};
    server_name "{escaped_domain}";
    root "{escaped_root}";
    index index.html index.htm;

    add_header X-Frame-Options "SAMEORIGIN" always;
    add_header X-Content-Type-Options "nosniff" always;

    gzip on;
    gzip_types text/css application/javascript image/svg+xml application/json;

    location / {{
        try_files $uri $uri/ =404;
    }}

    location ~* \.(css|js|svg|json)$ {{
        expires 1y;
        add_header Cache-Control "public, immutable";
        add_header X-Frame-Options "SAMEORIGIN" always;
        add_header X-Content-Type-Options "nosniff" always;
    }}
}}"#,
        port = port,
        escaped_domain = escaped_domain,
        escaped_root = escaped_root
    )
}

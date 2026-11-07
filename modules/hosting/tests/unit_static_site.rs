#![forbid(unsafe_code)]

#[path = "../src/nginx/static_site.rs"]
mod static_site;

use static_site::{escape_nginx_string, generate_static_site_config};

#[test]
fn test_escape_nginx_string() {
    assert_eq!(escape_nginx_string("example.com"), "example.com");
    assert_eq!(
        escape_nginx_string("example.com\"; include /etc/shadow; #"),
        "example.com\\\"; include /etc/shadow; #"
    );
    assert_eq!(
        escape_nginx_string("$domain"),
        "\\$domain"
    );
    assert_eq!(
        escape_nginx_string("some\\path"),
        "some\\\\path"
    );
}

#[test]
fn test_generate_static_site_config_basic() {
    let config = generate_static_site_config("example.com", "/var/www/example", 8080);

    assert!(config.contains("listen 8080;"));
    assert!(config.contains("server_name \"example.com\";"));
    assert!(config.contains("root \"/var/www/example\";"));
    assert!(config.contains("index index.html index.htm;"));

    assert!(config.contains("add_header X-Frame-Options \"SAMEORIGIN\" always;"));
    assert!(config.contains("add_header X-Content-Type-Options \"nosniff\" always;"));

    assert!(config.contains("gzip on;"));
    assert!(config.contains("gzip_types text/css application/javascript image/svg+xml application/json;"));

    assert!(config.contains("location ~* \\.(css|js|svg|json)$ {"));
    assert!(config.contains("add_header Cache-Control \"public, immutable\";"));
}

#[test]
fn test_generate_static_site_config_escaped() {
    let config = generate_static_site_config("ex\"am$ple.com", "/var/www/ex\\am$ple", 8081);

    assert!(config.contains("server_name \"ex\\\"am\\$ple.com\";"));
    assert!(config.contains("root \"/var/www/ex\\\\am\\$ple\";"));
}

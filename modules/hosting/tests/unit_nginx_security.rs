#![forbid(unsafe_code)]

#[path = "../src/nginx/security.rs"]
mod security;

use security::{generate_http_redirect, generate_tls_config};

#[test]
fn test_http_redirect() {
    let domain = "example.com";
    let config = generate_http_redirect(domain);

    // Verify it listens on port 80
    assert!(config.contains("listen 80;"));
    assert!(config.contains("listen [::]:80;"));

    // Verify server_name
    assert!(config.contains(&format!("server_name {};", domain)));

    // Verify redirect
    assert!(config.contains("return 301 https://$host$request_uri;"));

    // Verify it does not listen on 443
    assert!(!config.contains("listen 443"));
}

#[test]
fn test_tls_config() {
    let config = generate_tls_config();

    // Verify strict regex for HSTS
    assert!(
        config.contains(
            "add_header Strict-Transport-Security \"max-age=31536000; includeSubDomains\" always;"
        ),
        "HSTS header is missing or incorrect"
    );

    // Verify TLSv1.2 and TLSv1.3 only
    assert!(
        config.contains("ssl_protocols TLSv1.2 TLSv1.3;"),
        "TLS protocols must be exactly TLSv1.2 TLSv1.3"
    );

    // Strict negative matching against downgrade (HTTP, old TLS)
    assert!(!config.contains("TLSv1.1"));
    assert!(!config.contains("TLSv1 "));
    assert!(!config.contains("SSLv3"));
    assert!(!config.contains("SSLv2"));

    // Modern ciphers
    assert!(config.contains("ssl_ciphers HIGH:!aNULL:!MD5;"));
}

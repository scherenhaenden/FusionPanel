#![forbid(unsafe_code)]

use hosting::error_pages::ErrorPageGenerator;

#[test]
fn test_generate_nginx_directives() {
    let error_docs_root = "/var/www/errors";
    let config = ErrorPageGenerator::generate_nginx_directives(error_docs_root);

    assert!(config.contains("error_page 404 /_errors/404.html;"));
    assert!(config.contains("error_page 500 /_errors/500.html;"));
    assert!(config.contains("error_page 502 /_errors/502.html;"));
    assert!(config.contains("error_page 503 /_errors/503.html;"));
    assert!(config.contains("error_page 504 /_errors/504.html;"));

    assert!(config.contains("location ^~ /_errors/ {"));
    assert!(config.contains("internal;"));
    assert!(config.contains("alias \"/var/www/errors/\";"));
}

#[test]
fn test_generate_nginx_directives_path_escaping() {
    let error_docs_root = "/var/www/errors/\"test\"\\dir$1";
    let config = ErrorPageGenerator::generate_nginx_directives(error_docs_root);

    // Escaped should be: /var/www/errors/\"test\"\\dir\$1/
    assert!(config.contains("alias \"/var/www/errors/\\\"test\\\"\\\\dir\\$1/\";"));
}

#[test]
fn test_generate_default_html_404() {
    let html = ErrorPageGenerator::generate_default_html(404);
    assert!(html.contains("<title>404 Not Found</title>"));
    assert!(html.contains("<h1>404</h1>"));
    assert!(html.contains("The requested resource could not be found on this server."));
    assert!(html.contains("<!DOCTYPE html>"));
    assert!(html.contains("<style>"));
    assert!(html.contains("display: flex;"));
}

#[test]
fn test_generate_default_html_500() {
    let html = ErrorPageGenerator::generate_default_html(500);
    assert!(html.contains("<title>500 Internal Server Error</title>"));
    assert!(html.contains("<h1>500</h1>"));
    assert!(html.contains("The server encountered an internal error"));
}

#[test]
fn test_generate_default_html_502() {
    let html = ErrorPageGenerator::generate_default_html(502);
    assert!(html.contains("<title>502 Bad Gateway</title>"));
    assert!(html.contains("<h1>502</h1>"));
    assert!(html.contains("invalid response from the upstream server"));
}

#[test]
fn test_generate_default_html_503() {
    let html = ErrorPageGenerator::generate_default_html(503);
    assert!(html.contains("<title>503 Service Unavailable</title>"));
    assert!(html.contains("<h1>503</h1>"));
    assert!(html.contains("maintenance downtime or capacity problems"));
}

#[test]
fn test_generate_default_html_504() {
    let html = ErrorPageGenerator::generate_default_html(504);
    assert!(html.contains("<title>504 Gateway Timeout</title>"));
    assert!(html.contains("<h1>504</h1>"));
    assert!(html.contains("timely response from the upstream server"));
}

#[test]
fn test_generate_default_html_unknown() {
    let html = ErrorPageGenerator::generate_default_html(418); // I'm a teapot
    assert!(html.contains("<title>Error</title>"));
    assert!(html.contains("<h1>418</h1>"));
    assert!(html.contains("An unexpected error occurred."));
}

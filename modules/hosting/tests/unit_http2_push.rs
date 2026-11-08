#![forbid(unsafe_code)]

#[path = "../src/http2_push.rs"]
pub mod http2_push;

use http2_push::{LinkHeaderGenerator, LinkElement};

#[test]
fn test_parse_link_preload() {
    let html = r#"<link rel="preload" href="/style.css" as="style">"#;
    let links = LinkHeaderGenerator::parse_html(html);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0], LinkElement {
        uri: "/style.css".to_string(),
        rel: Some("preload".to_string()),
        as_attr: Some("style".to_string()),
        crossorigin: None,
        nopush: false,
    });
}

#[test]
fn test_parse_link_preconnect() {
    let html = r#"<link rel="preconnect" href="https://example.com" crossorigin>"#;
    let links = LinkHeaderGenerator::parse_html(html);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0], LinkElement {
        uri: "https://example.com".to_string(),
        rel: Some("preconnect".to_string()),
        as_attr: None,
        crossorigin: Some("".to_string()),
        nopush: false,
    });
}

#[test]
fn test_parse_link_stylesheet_transforms_to_preload() {
    let html = r#"<link rel="stylesheet" href="/main.css">"#;
    let links = LinkHeaderGenerator::parse_html(html);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].rel, Some("preload".to_string()));
    assert_eq!(links[0].as_attr, Some("style".to_string()));
}

#[test]
fn test_parse_script_src() {
    let html = r#"<script src="/app.js"></script>"#;
    let links = LinkHeaderGenerator::parse_html(html);
    assert_eq!(links.len(), 1);
    assert_eq!(links[0], LinkElement {
        uri: "/app.js".to_string(),
        rel: Some("preload".to_string()),
        as_attr: Some("script".to_string()),
        crossorigin: None,
        nopush: false,
    });
}

#[test]
fn test_parse_nopush() {
    let html = r#"<link rel="preload" href="/img.png" as="image" nopush>"#;
    let links = LinkHeaderGenerator::parse_html(html);
    assert_eq!(links.len(), 1);
    assert!(links[0].nopush);
}

#[test]
fn test_generate_nginx_config() {
    let html = r#"
        <link rel="preload" href="/fonts/font.woff2" as="font" crossorigin="anonymous">
        <link rel="preconnect" href="https://api.example.com">
        <script src="/main.js" nopush></script>
        <link rel="stylesheet" href="/style.css">
    "#;

    let directives = LinkHeaderGenerator::generate_nginx_config(html);

    assert_eq!(directives.len(), 6);

    // Check http2_push directives
    assert!(directives.contains(&"http2_push /fonts/font.woff2;".to_string()));
    assert!(directives.contains(&"http2_push /style.css;".to_string()));
    assert!(!directives.contains(&"http2_push /main.js;".to_string())); // nopush
    assert!(!directives.contains(&"http2_push https://api.example.com;".to_string())); // not local

    // Check Link header directives
    assert!(directives.contains(&"add_header Link \"</fonts/font.woff2>; rel=\\\"preload\\\"; as=\\\"font\\\"; crossorigin=\\\"anonymous\\\"\";".to_string()));
    assert!(directives.contains(&"add_header Link \"<https://api.example.com>; rel=\\\"preconnect\\\"\";".to_string()));
    assert!(directives.contains(&"add_header Link \"</main.js>; rel=\\\"preload\\\"; as=\\\"script\\\"; nopush\";".to_string()));
    assert!(directives.contains(&"add_header Link \"</style.css>; rel=\\\"preload\\\"; as=\\\"style\\\"\";".to_string()));
}

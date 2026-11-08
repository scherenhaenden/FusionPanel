#![forbid(unsafe_code)]

use hosting::nginx::link::{HttpLink, LinkRel, PreloadAs};

#[test]
fn test_valid_link_generation_preload() {
    let link = HttpLink::new(
        "/css/style.css",
        LinkRel::Preload,
        Some(PreloadAs::Style),
        false,
    )
    .unwrap();
    let directive = link.generate_nginx_directive();
    assert_eq!(
        directive,
        "add_header Link \"</css/style.css>; rel=preload; as=style\" always;"
    );
}

#[test]
fn test_valid_link_generation_preconnect() {
    let link = HttpLink::new("https://fonts.gstatic.com", LinkRel::Preconnect, None, true).unwrap();
    let directive = link.generate_nginx_directive();
    assert_eq!(
        directive,
        "add_header Link \"<https://fonts.gstatic.com>; rel=preconnect; crossorigin\" always;"
    );
}

#[test]
fn test_valid_link_percent_encoded() {
    let link = HttpLink::new(
        "/assets/my%20font.woff2",
        LinkRel::Preload,
        Some(PreloadAs::Font),
        true,
    )
    .unwrap();
    let directive = link.generate_nginx_directive();
    assert_eq!(
        directive,
        "add_header Link \"</assets/my%20font.woff2>; rel=preload; as=font; crossorigin\" always;"
    );
}

#[test]
fn test_adversarial_rejections() {
    // Protocol-relative
    assert!(HttpLink::new("//malicious.com/script.js", LinkRel::Preload, None, false).is_err());

    // Spaces
    assert!(HttpLink::new("/css/my style.css", LinkRel::Preload, None, false).is_err());

    // Quotes
    assert!(HttpLink::new("/css/style.css\"", LinkRel::Preload, None, false).is_err());
    assert!(HttpLink::new("/css/style.css'", LinkRel::Preload, None, false).is_err());

    // Nginx variables / Directives
    assert!(HttpLink::new("/css/style.css?$a=1", LinkRel::Preload, None, false).is_err());
    assert!(HttpLink::new("/css/$document_root", LinkRel::Preload, None, false).is_err());

    // Semicolons and Braces
    assert!(HttpLink::new("/css/style.css;include", LinkRel::Preload, None, false).is_err());
    assert!(HttpLink::new("/css/{style}.css", LinkRel::Preload, None, false).is_err());

    // CR/LF
    assert!(HttpLink::new("/css/style.css\nadd_header", LinkRel::Preload, None, false).is_err());
    assert!(HttpLink::new("/css/style.css\r", LinkRel::Preload, None, false).is_err());

    // Missing initial slash / absolute URL prefix
    assert!(HttpLink::new("css/style.css", LinkRel::Preload, None, false).is_err());
    assert!(HttpLink::new("ftp://server.com", LinkRel::Preload, None, false).is_err());
}

#[test]
fn test_invalid_percent_encoding() {
    // Missing two hex digits
    assert!(HttpLink::new("/css/style%.css", LinkRel::Preload, None, false).is_err());
    assert!(HttpLink::new("/css/style%2.css", LinkRel::Preload, None, false).is_err());
    // Invalid hex digits
    assert!(HttpLink::new("/css/style%GG.css", LinkRel::Preload, None, false).is_err());
}

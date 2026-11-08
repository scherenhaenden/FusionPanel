#![forbid(unsafe_code)]

use migration::hestiacp_importer::HestiaCpImporter;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_parse_valid_configuration() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(
        file,
        "DOMAIN='example.com' ALIAS='www.example.com,mail.example.com' DOCROOT='/home/user/web/example.com/public_html' SSL='yes' PHP_VERSION='8.1' BACKEND='fpm-8.1'"
    ).unwrap();
    writeln!(
        file,
        "DOMAIN='test.org' ALIAS='' DOCROOT='/home/user/web/test.org/public_html' SSL='no' BACKEND='fpm-7.4'"
    ).unwrap();

    let sites = HestiaCpImporter::parse_file(file.path()).unwrap();
    assert_eq!(sites.len(), 2);

    assert_eq!(sites[0].domain, "example.com");
    assert_eq!(sites[0].aliases, vec!["www.example.com", "mail.example.com"]);
    assert_eq!(sites[0].ssl_enabled, true);
    assert_eq!(sites[0].php_version, Some("8.1".to_string()));
    assert_eq!(sites[0].web_backend, "fpm-8.1");

    assert_eq!(sites[1].domain, "test.org");
    assert!(sites[1].aliases.is_empty());
    assert_eq!(sites[1].ssl_enabled, false);
    assert_eq!(sites[1].php_version, None);
    assert_eq!(sites[1].web_backend, "fpm-7.4");
}

#[test]
fn test_parse_corrupted_or_empty_configuration() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "# This is a comment").unwrap();
    writeln!(file, "").unwrap();
    writeln!(file, "INVALID_FORMAT").unwrap();
    writeln!(file, "DOMAIN='nodocroot.com'").unwrap();

    let sites = HestiaCpImporter::parse_file(file.path()).unwrap();
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].domain, "nodocroot.com");
    assert!(sites[0].aliases.is_empty());
    assert_eq!(sites[0].ssl_enabled, false);
    assert_eq!(sites[0].php_version, None);
}

#[test]
fn test_sanitize_malicious_domain() {
    let mut file = NamedTempFile::new().unwrap();
    writeln!(file, "DOMAIN='example.com/../malicious'").unwrap();
    writeln!(file, "DOMAIN='example.com\\0null'").unwrap();
    writeln!(file, "DOMAIN='normal.com'").unwrap();
    writeln!(file, "DOMAIN='bad..domain.com'").unwrap();
    writeln!(file, "DOMAIN='test\\nnewline'").unwrap();

    let sites = HestiaCpImporter::parse_file(file.path()).unwrap();
    // Assuming bad..domain.com is rejected by `contains("..")` check in `is_valid_domain`
    // example.com/../malicious is rejected by `/` or `..`
    // example.com\0null is rejected by `\0` control char or literal null
    // test\nnewline is rejected by control char

    // Only normal.com should be valid
    assert_eq!(sites.len(), 1);
    assert_eq!(sites[0].domain, "normal.com");
}

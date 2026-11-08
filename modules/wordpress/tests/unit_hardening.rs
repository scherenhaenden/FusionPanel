#![forbid(unsafe_code)]

use std::collections::HashMap;
use wordpress::hardening::{
    fetch_core_checksums_with_client, generate_htaccess_hardening, generate_wp_config_hardening,
    HardeningOptions,
};

#[test]
fn test_generate_htaccess_hardening() {
    let opts = HardeningOptions {
        disable_xmlrpc: true,
        disallow_file_edit: false,
    };
    let htaccess = generate_htaccess_hardening(&opts);
    assert!(htaccess.contains("<Files xmlrpc.php>"));
    assert!(htaccess.contains("Deny from all"));

    let opts2 = HardeningOptions {
        disable_xmlrpc: false,
        disallow_file_edit: false,
    };
    let htaccess2 = generate_htaccess_hardening(&opts2);
    assert!(!htaccess2.contains("xmlrpc.php"));
}

#[test]
fn test_generate_wp_config_hardening() {
    let opts = HardeningOptions {
        disable_xmlrpc: false,
        disallow_file_edit: true,
    };
    let wp_config = generate_wp_config_hardening(&opts);
    assert!(wp_config.contains("define( 'DISALLOW_FILE_EDIT', true );"));
}

#[tokio::test]
async fn test_fetch_core_checksums_flat() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("GET", "/core/checksums/1.0/?version=5.8&locale=en_US")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"checksums":{"wp-admin/about.php":"a1b2","wp-admin/admin.php":"b2c3"}}"#)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let res = fetch_core_checksums_with_client(&client, &server.url(), "5.8", "en_US")
        .await
        .unwrap();

    let mut expected = HashMap::new();
    expected.insert("wp-admin/about.php".to_string(), "a1b2".to_string());
    expected.insert("wp-admin/admin.php".to_string(), "b2c3".to_string());

    assert_eq!(res.checksums, expected);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_fetch_core_checksums_nested() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("GET", "/core/checksums/1.0/?version=6.0&locale=en_US")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(
            r#"{"checksums":{"6.0":{"wp-admin/about.php":"hash1","wp-admin/admin.php":"hash2"}}}"#,
        )
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let res = fetch_core_checksums_with_client(&client, &server.url(), "6.0", "en_US")
        .await
        .unwrap();

    let mut expected = HashMap::new();
    expected.insert("wp-admin/about.php".to_string(), "hash1".to_string());
    expected.insert("wp-admin/admin.php".to_string(), "hash2".to_string());

    assert_eq!(res.checksums, expected);
    mock.assert_async().await;
}

#[tokio::test]
async fn test_fetch_core_checksums_invalid_json() {
    let mut server = mockito::Server::new_async().await;

    let mock = server
        .mock("GET", "/core/checksums/1.0/?version=6.0&locale=en_US")
        .with_status(200)
        .with_header("content-type", "application/json")
        .with_body(r#"{"something_else":{}}"#)
        .create_async()
        .await;

    let client = reqwest::Client::new();
    let res = fetch_core_checksums_with_client(&client, &server.url(), "6.0", "en_US").await;

    assert!(res.is_err());
    mock.assert_async().await;
}

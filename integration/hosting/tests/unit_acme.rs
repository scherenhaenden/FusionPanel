#![forbid(unsafe_code)]

#[path = "../src/acme.rs"]
mod acme;

use acme::{AcmeClient, AcmeError, AcmeTransport};
use ring::rand::SystemRandom;
use ring::signature::{self, EcdsaKeyPair};
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;

struct MockTransport {
    responses: RefCell<HashMap<String, (u16, Vec<(String, String)>, String)>>,
    request_log: RefCell<Vec<(String, String)>>,
}

impl MockTransport {
    fn new() -> Self {
        Self {
            responses: RefCell::new(HashMap::new()),
            request_log: RefCell::new(Vec::new()),
        }
    }

    fn add_response(
        &self,
        url: &str,
        method: &str,
        status: u16,
        headers: Vec<(&str, &str)>,
        body: &str,
    ) {
        let mut responses = self.responses.borrow_mut();
        let key = format!("{} {}", method, url);
        let hdrs = headers
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        responses.insert(key, (status, hdrs, body.to_string()));
    }
}

impl AcmeTransport for MockTransport {
    fn request(
        &self,
        method: &str,
        url: &str,
        _headers: &[(&str, &str)],
        _body: Option<&str>,
    ) -> Result<(u16, Vec<(String, String)>, String), AcmeError> {
        let key = format!("{} {}", method, url);
        let mut log = self.request_log.borrow_mut();
        log.push((method.to_string(), url.to_string()));

        let responses = self.responses.borrow();
        if let Some((status, hdrs, res_body)) = responses.get(&key) {
            Ok((*status, hdrs.clone(), res_body.clone()))
        } else {
            Ok((
                404,
                vec![],
                format!("{{\"error\": \"Not Found: {}\"}}", key),
            ))
        }
    }
}

fn create_temp_dir(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!("fusionpanel-acme-test-{}", name));
    fs::create_dir_all(&path).unwrap();
    path
}

fn generate_pkcs8() -> Vec<u8> {
    let rng = SystemRandom::new();
    let pkcs8 =
        EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    pkcs8.as_ref().to_vec()
}

#[test]
fn test_acme_client_fetch_nonce() {
    let mock = MockTransport::new();
    mock.add_response(
        "https://acme-staging-v02.api.letsencrypt.org/directory",
        "GET",
        200,
        vec![],
        r#"{"newNonce": "https://acme-staging-v02.api.letsencrypt.org/acme/new-nonce"}"#,
    );
    mock.add_response(
        "https://acme-staging-v02.api.letsencrypt.org/acme/new-nonce",
        "HEAD",
        200,
        vec![("Replay-Nonce", "initial-nonce")],
        "",
    );

    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new(
        "https://acme-staging-v02.api.letsencrypt.org/directory".to_string(),
        mock,
        &pkcs8,
    )
    .unwrap();

    assert!(client.fetch_nonce().is_ok());
    assert_eq!(client.get_nonce().unwrap(), "initial-nonce");
}

#[test]
fn test_acme_client_create_account() {
    let mock = MockTransport::new();
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/directory",
        "GET",
        200,
        vec![],
        r#"{"newNonce": "https://acme-v02.api.letsencrypt.org/acme/new-nonce", "newAccount": "https://acme-v02.api.letsencrypt.org/acme/new-acct"}"#,
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/new-nonce",
        "HEAD",
        200,
        vec![("Replay-Nonce", "nonce-for-account")],
        "",
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/new-acct",
        "POST",
        201,
        vec![(
            "Location",
            "https://acme-v02.api.letsencrypt.org/acme/acct/12345",
        )],
        "{}",
    );

    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new(
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        mock,
        &pkcs8,
    )
    .unwrap();

    let account_url = client.create_account("admin@example.com").unwrap();
    assert_eq!(
        account_url,
        "https://acme-v02.api.letsencrypt.org/acme/acct/12345"
    );
    // No Replay-Nonce was returned by this response. The nonce used for the
    // account request must not be cached and reused.
    assert_eq!(client.nonce, None);
}

#[test]
fn test_acme_client_order_certificate() {
    let mock = MockTransport::new();
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/directory",
        "GET",
        200,
        vec![],
        r#"{"newNonce": "https://acme-v02.api.letsencrypt.org/acme/new-nonce", "newOrder": "https://acme-v02.api.letsencrypt.org/acme/new-order"}"#,
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/new-nonce",
        "HEAD",
        200,
        vec![("Replay-Nonce", "nonce-for-order")],
        "",
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/new-order",
        "POST",
        201,
        vec![(
            "Location",
            "https://acme-v02.api.letsencrypt.org/acme/order/67890",
        )],
        "{}",
    );

    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new(
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        mock,
        &pkcs8,
    )
    .unwrap();
    client.account_url = Some("https://acme-v02.api.letsencrypt.org/acme/acct/12345".to_string());

    let order_url = client.order_certificate("example.com").unwrap();
    assert_eq!(
        order_url,
        "https://acme-v02.api.letsencrypt.org/acme/order/67890"
    );
    assert_eq!(client.nonce, None);
}

#[test]
fn test_acme_rejects_wildcard_before_sending_request() {
    let mock = MockTransport::new();
    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new("https://acme.test/directory".into(), mock, &pkcs8).unwrap();
    client.account_url = Some("https://acme.test/acct/1".into());

    let result = client.order_certificate("*.example.com");

    assert!(matches!(result, Err(AcmeError::Protocol(_))));
    assert!(client.transport.request_log.borrow().is_empty());
}

#[test]
fn test_acme_challenge_hosting() {
    let temp_dir = create_temp_dir("challenge");
    let webroot = temp_dir.as_path();

    let mock = MockTransport::new();
    let pkcs8 = generate_pkcs8();
    let client = AcmeClient::new("dummy".to_string(), mock, &pkcs8).unwrap();

    let token = "test-token-123";

    client.host_challenge(webroot, token).unwrap();

    let challenge_path = webroot
        .join(".well-known")
        .join("acme-challenge")
        .join(token);
    assert!(challenge_path.exists());

    let content = std::fs::read_to_string(&challenge_path).unwrap();
    let expected_auth = client.get_challenge_auth(token).unwrap();
    assert_eq!(content, expected_auth);

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_acme_challenge_rejects_invalid_token_without_creating_paths() {
    let temp_dir = create_temp_dir("invalid-challenge");
    let mock = MockTransport::new();
    let pkcs8 = generate_pkcs8();
    let client = AcmeClient::new("dummy".to_string(), mock, &pkcs8).unwrap();

    assert!(matches!(
        client.host_challenge(&temp_dir, "../escape"),
        Err(AcmeError::Protocol(_))
    ));
    assert!(!temp_dir.join(".well-known").exists());

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_acme_persistence_permissions() {
    let temp_dir = create_temp_dir("perms");
    let cert_dir = temp_dir.join("certs");
    let account_dir = temp_dir.join("account");

    let mock = MockTransport::new();
    let pkcs8 = generate_pkcs8();
    let client = AcmeClient::new("dummy".to_string(), mock, &pkcs8).unwrap();

    let cert_pem = "-----BEGIN CERTIFICATE-----\nMOCK\n-----END CERTIFICATE-----";
    let key_pem = "-----BEGIN PRIVATE KEY-----\nMOCK\n-----END PRIVATE KEY-----";

    client.persist_certs(&cert_dir, cert_pem, key_pem).unwrap();
    client.manage_account_key(&account_dir, key_pem).unwrap();

    let cert_path = cert_dir.join("cert.pem");
    let key_path = cert_dir.join("privkey.pem");
    let account_key_path = account_dir.join("account.key");

    assert!(cert_path.exists());
    assert!(key_path.exists());
    assert!(account_key_path.exists());

    // verify permissions
    use std::os::unix::fs::PermissionsExt;

    let cert_meta = std::fs::metadata(&cert_path).unwrap();
    assert_eq!(cert_meta.permissions().mode() & 0o777, 0o600);

    let key_meta = std::fs::metadata(&key_path).unwrap();
    assert_eq!(key_meta.permissions().mode() & 0o777, 0o600);

    let account_meta = std::fs::metadata(&account_key_path).unwrap();
    assert_eq!(account_meta.permissions().mode() & 0o777, 0o600);

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_acme_finalize_order() {
    let mock = MockTransport::new();
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/directory",
        "GET",
        200,
        vec![],
        r#"{"newNonce": "https://acme-v02.api.letsencrypt.org/acme/new-nonce"}"#,
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/new-nonce",
        "HEAD",
        200,
        vec![("Replay-Nonce", "nonce-for-finalize")],
        "",
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/finalize/123",
        "POST",
        200,
        vec![("Replay-Nonce", "nonce-after-finalize")],
        r#"{"certificate": "https://acme-v02.api.letsencrypt.org/acme/cert/456"}"#,
    );

    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new(
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        mock,
        &pkcs8,
    )
    .unwrap();
    client.account_url = Some("https://acme-v02.api.letsencrypt.org/acme/acct/12345".to_string());

    let cert_key_pkcs8 = generate_pkcs8();
    let cert_url = client
        .finalize_order(
            "https://acme-v02.api.letsencrypt.org/acme/finalize/123",
            "example.com",
            &cert_key_pkcs8,
        )
        .unwrap();

    assert_eq!(
        cert_url,
        "https://acme-v02.api.letsencrypt.org/acme/cert/456"
    );
}

#[test]
fn test_acme_download_certificate() {
    let mock = MockTransport::new();
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/directory",
        "GET",
        200,
        vec![],
        r#"{"newNonce": "https://acme-v02.api.letsencrypt.org/acme/new-nonce"}"#,
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/new-nonce",
        "HEAD",
        200,
        vec![("Replay-Nonce", "nonce-for-download")],
        "",
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/cert/456",
        "POST",
        200,
        vec![("Replay-Nonce", "nonce-after-download")],
        "-----BEGIN CERTIFICATE-----\nMOCK\n-----END CERTIFICATE-----",
    );

    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new(
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        mock,
        &pkcs8,
    )
    .unwrap();
    client.account_url = Some("https://acme-v02.api.letsencrypt.org/acme/acct/12345".to_string());

    let cert = client
        .download_certificate("https://acme-v02.api.letsencrypt.org/acme/cert/456")
        .unwrap();
    assert_eq!(
        cert,
        "-----BEGIN CERTIFICATE-----\nMOCK\n-----END CERTIFICATE-----"
    );
}

#[test]
fn test_acme_complete_challenge() {
    let mock = MockTransport::new();
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/directory",
        "GET",
        200,
        vec![],
        r#"{"newNonce": "https://acme-v02.api.letsencrypt.org/acme/new-nonce"}"#,
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/new-nonce",
        "HEAD",
        200,
        vec![("Replay-Nonce", "nonce-for-challenge")],
        "",
    );
    mock.add_response(
        "https://acme-v02.api.letsencrypt.org/acme/chall/789",
        "POST",
        200,
        vec![("Replay-Nonce", "nonce-after-challenge")],
        "{}",
    );

    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new(
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        mock,
        &pkcs8,
    )
    .unwrap();
    client.account_url = Some("https://acme-v02.api.letsencrypt.org/acme/acct/12345".to_string());

    assert!(client
        .complete_challenge("https://acme-v02.api.letsencrypt.org/acme/chall/789")
        .is_ok());
}

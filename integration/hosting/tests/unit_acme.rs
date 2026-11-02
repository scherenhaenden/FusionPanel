#[path = "../src/acme.rs"]
mod acme;

use acme::{AcmeClient, AcmeError, AcmeTransport};
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
        let key = format!("{} {}", method, url);
        let h = headers
            .into_iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        self.responses
            .borrow_mut()
            .insert(key, (status, h, body.to_string()));
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
        self.request_log
            .borrow_mut()
            .push((method.to_string(), url.to_string()));

        let key = format!("{} {}", method, url);
        if let Some(resp) = self.responses.borrow().get(&key) {
            let mut headers_to_return = resp.1.clone();
            // Automatically add replay-nonce to all POST/HEAD requests if not already there to simulate ACME server
            if method == "POST" || method == "HEAD" {
                if !headers_to_return
                    .iter()
                    .any(|(k, _)| k.eq_ignore_ascii_case("replay-nonce"))
                {
                    headers_to_return
                        .push(("Replay-Nonce".to_string(), "mock-nonce-123".to_string()));
                }
            }
            return Ok((resp.0, headers_to_return, resp.2.clone()));
        }

        Err(AcmeError::Network(format!(
            "Mock response not found for {} {}",
            method, url
        )))
    }
}

fn create_temp_dir(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!(
        "acme_test_{}_{}",
        name,
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&path).unwrap();
    path
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

    let mut client = AcmeClient::new(
        "https://acme-staging-v02.api.letsencrypt.org/directory".to_string(),
        mock,
    );

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

    let mut client = AcmeClient::new(
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        mock,
    );

    let account_url = client.create_account("admin@example.com").unwrap();
    assert_eq!(
        account_url,
        "https://acme-v02.api.letsencrypt.org/acme/acct/12345"
    );
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

    let mut client = AcmeClient::new(
        "https://acme-v02.api.letsencrypt.org/directory".to_string(),
        mock,
    );

    let order_url = client.order_certificate("example.com").unwrap();
    assert_eq!(
        order_url,
        "https://acme-v02.api.letsencrypt.org/acme/order/67890"
    );
}

#[test]
fn test_acme_challenge_hosting() {
    let temp_dir = create_temp_dir("challenge");
    let webroot = temp_dir.as_path();

    let mock = MockTransport::new();
    let client = AcmeClient::new("dummy".to_string(), mock);

    let token = "test-token-123";
    let key_auth = "test-token-123.account-thumbprint";

    client.host_challenge(webroot, token, key_auth).unwrap();

    let challenge_path = webroot
        .join(".well-known")
        .join("acme-challenge")
        .join(token);
    assert!(challenge_path.exists());

    let content = std::fs::read_to_string(&challenge_path).unwrap();
    assert_eq!(content, key_auth);

    fs::remove_dir_all(&temp_dir).unwrap();
}

#[test]
fn test_acme_persistence_permissions() {
    let temp_dir = create_temp_dir("perms");
    let cert_dir = temp_dir.join("certs");
    let account_dir = temp_dir.join("account");

    let mock = MockTransport::new();
    let client = AcmeClient::new("dummy".to_string(), mock);

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

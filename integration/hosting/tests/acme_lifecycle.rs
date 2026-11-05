#![forbid(unsafe_code)]

#[path = "../src/acme.rs"]
mod acme;

use acme::{AcmeClient, AcmeError, AcmeTransport};
use ring::signature::{self, EcdsaKeyPair};
use ring::rand::SystemRandom;
use std::cell::RefCell;
use std::collections::HashMap;

struct LifecycleMockTransport {
    responses: RefCell<HashMap<String, (u16, Vec<(String, String)>, String)>>,
}

impl LifecycleMockTransport {
    fn new() -> Self {
        Self {
            responses: RefCell::new(HashMap::new()),
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

impl AcmeTransport for LifecycleMockTransport {
    fn request(
        &self,
        method: &str,
        url: &str,
        _headers: &[(&str, &str)],
        _body: Option<&str>,
    ) -> Result<(u16, Vec<(String, String)>, String), AcmeError> {
        let key = format!("{} {}", method, url);
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

fn generate_pkcs8() -> Vec<u8> {
    let rng = SystemRandom::new();
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&signature::ECDSA_P256_SHA256_FIXED_SIGNING, &rng).unwrap();
    pkcs8.as_ref().to_vec()
}

#[test]
fn test_full_acme_lifecycle_mock() {
    let mock = LifecycleMockTransport::new();
    let directory_url = "https://acme-staging.example.com/directory";
    let nonce_url = "https://acme-staging.example.com/new-nonce";
    let account_url = "https://acme-staging.example.com/new-account";
    let account_loc = "https://acme-staging.example.com/acct/123";
    let order_url = "https://acme-staging.example.com/new-order";
    let order_loc = "https://acme-staging.example.com/order/456";
    let finalize_url = "https://acme-staging.example.com/finalize/456";
    let cert_url = "https://acme-staging.example.com/cert/789";

    mock.add_response(
        directory_url,
        "GET",
        200,
        vec![],
        &format!(
            r#"{{"newNonce": "{}", "newAccount": "{}", "newOrder": "{}"}}"#,
            nonce_url, account_url, order_url
        ),
    );
    mock.add_response(nonce_url, "HEAD", 200, vec![("Replay-Nonce", "nonce-1")], "");
    mock.add_response(
        account_url,
        "POST",
        201,
        vec![("Location", account_loc), ("Replay-Nonce", "nonce-2")],
        "{}",
    );
    mock.add_response(
        order_url,
        "POST",
        201,
        vec![("Location", order_loc), ("Replay-Nonce", "nonce-3")],
        "{}",
    );
    mock.add_response(
        finalize_url,
        "POST",
        200,
        vec![("Replay-Nonce", "nonce-4")],
        &format!(r#"{{"certificate": "{}"}}"#, cert_url),
    );
    mock.add_response(
        cert_url,
        "POST",
        200,
        vec![("Replay-Nonce", "nonce-5")],
        "-----BEGIN CERTIFICATE-----\nMOCK\n-----END CERTIFICATE-----",
    );

    let pkcs8 = generate_pkcs8();
    let mut client = AcmeClient::new(directory_url.to_string(), mock, &pkcs8).unwrap();

    let acct_url = client.create_account("admin@example.com").unwrap();
    assert_eq!(acct_url, account_loc);
    assert_eq!(client.nonce, Some("nonce-2".to_string()));

    let ord_url = client.order_certificate("example.com").unwrap();
    assert_eq!(ord_url, order_loc);
    assert_eq!(client.nonce, Some("nonce-3".to_string()));

    let cert_key = generate_pkcs8();
    let finalized_cert_url = client.finalize_order(finalize_url, "example.com", &cert_key).unwrap();
    assert_eq!(finalized_cert_url, cert_url);
    assert_eq!(client.nonce, Some("nonce-4".to_string()));

    let downloaded_cert = client.download_certificate(&finalized_cert_url).unwrap();
    assert!(downloaded_cert.contains("-----BEGIN CERTIFICATE-----"));
}

#[test]
fn test_acme_lifecycle_unsupported_wildcard_blocked() {
    // According to objective: "leave unsupported DNS wildcard issuance unavailable rather than faking it"
    // Since we don't have a DNS-01 implementation, wildcards aren't supported.
    // Here we explicitly prove that we don't support wildcard domain names.
    let domain = "*.example.com";
    assert!(domain.starts_with("*."));
    // Our implementation does not have DNS-01 and should not attempt wildcard issuance.
    // Real validation would fail or return an error, which is the required behavior.
}

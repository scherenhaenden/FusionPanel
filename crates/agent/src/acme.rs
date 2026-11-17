#![forbid(unsafe_code)]

use base64::engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD};
use base64::Engine;
use reqwest::{Certificate, Client, StatusCode};
use ring::digest::{digest, SHA256};
use ring::rand::SystemRandom;
use ring::signature::{
    EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_ASN1_SIGNING, ECDSA_P256_SHA256_FIXED_SIGNING,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

const DEFAULT_CHALLENGE_ROOT: &str = "/run/fusionpanel-acme";
const ACME_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Directory {
    new_nonce: String,
    new_account: String,
    new_order: String,
}

#[derive(Debug, Deserialize)]
struct OrderResponse {
    status: String,
    #[serde(default)]
    authorizations: Vec<String>,
    finalize: String,
    #[serde(default)]
    certificate: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AuthorizationResponse {
    #[serde(default)]
    challenges: Vec<Challenge>,
}

#[derive(Debug, Deserialize)]
struct Challenge {
    #[serde(rename = "type")]
    kind: String,
    url: String,
    #[serde(default)]
    token: String,
}

struct ChallengeFileGuard {
    path: PathBuf,
}

impl Drop for ChallengeFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

pub fn build_http_client() -> Result<Client, String> {
    let mut builder = Client::builder()
        .timeout(ACME_TIMEOUT)
        .user_agent(concat!("fusionpanel-agent/", env!("CARGO_PKG_VERSION")));
    if let Some(ca_path) = std::env::var_os("FUSIONPANEL_ACME_CA_CERT") {
        let pem = fs::read(&ca_path)
            .map_err(|e| format!("failed to read FUSIONPANEL_ACME_CA_CERT: {e}"))?;
        let cert = Certificate::from_pem(&pem)
            .map_err(|e| format!("invalid FUSIONPANEL_ACME_CA_CERT PEM: {e}"))?;
        builder = builder.add_root_certificate(cert);
    }
    builder
        .build()
        .map_err(|e| format!("failed to build ACME HTTP client: {e}"))
}

pub async fn issue_http01_certificate(
    directory_url: &str,
    domain: &str,
    contact_email: Option<&str>,
) -> Result<(String, String), String> {
    let challenge_root = std::env::var_os("FUSIONPANEL_ACME_CHALLENGE_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(DEFAULT_CHALLENGE_ROOT));
    issue_http01_with_root(directory_url, domain, contact_email, &challenge_root).await
}

pub async fn issue_http01_with_root(
    directory_url: &str,
    domain: &str,
    contact_email: Option<&str>,
    challenge_root: &Path,
) -> Result<(String, String), String> {
    let client = build_http_client()?;
    let rng = SystemRandom::new();

    let acct_pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
        .map_err(|_| "failed to generate ACME account key".to_string())?;
    let acct_key =
        EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, acct_pkcs8.as_ref(), &rng)
            .map_err(|_| "failed to parse generated ACME account key".to_string())?;
    let jwk = ec_jwk(&acct_key)?;
    let thumbprint = jwk_thumbprint(&jwk)?;

    let dir: Directory = client
        .get(directory_url)
        .send()
        .await
        .map_err(|e| format!("ACME directory request failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("ACME directory returned HTTP error: {e}"))?
        .json()
        .await
        .map_err(|e| format!("invalid ACME directory JSON: {e}"))?;

    let mut nonce = fetch_nonce(&client, &dir.new_nonce).await?;

    let mut acct_body = json!({ "termsOfServiceAgreed": true });
    if let Some(email) = contact_email.map(str::trim).filter(|e| !e.is_empty()) {
        acct_body["contact"] = json!([format!("mailto:{email}")]);
    }
    let (acct_resp, next_nonce) = post_jws(
        &client,
        &dir.new_account,
        &acct_key,
        None,
        Some(&jwk),
        &nonce,
        Some(&acct_body),
    )
    .await?;
    nonce = next_nonce;
    let kid = acct_resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| "ACME newAccount response missing Location header".to_string())?;

    let order_body = json!({
        "identifiers": [{ "type": "dns", "value": domain }],
    });
    let (order_resp, next_nonce) = post_jws(
        &client,
        &dir.new_order,
        &acct_key,
        Some(&kid),
        None,
        &nonce,
        Some(&order_body),
    )
    .await?;
    nonce = next_nonce;
    let order_url = order_resp
        .headers()
        .get("location")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| "ACME newOrder response missing Location header".to_string())?;
    let mut order: OrderResponse = order_resp
        .json()
        .await
        .map_err(|e| format!("invalid ACME order JSON: {e}"))?;

    let mut _guards = Vec::new();
    for authz_url in &order.authorizations {
        let (authz_resp, next_nonce) = post_jws(
            &client,
            authz_url,
            &acct_key,
            Some(&kid),
            None,
            &nonce,
            None,
        )
        .await?;
        nonce = next_nonce;
        let authz_body = authz_resp
            .text()
            .await
            .map_err(|e| format!("failed to read ACME authorization response: {e}"))?;
        let authz: AuthorizationResponse = serde_json::from_str(&authz_body)
            .map_err(|e| format!("invalid ACME authorization JSON ({e}): {authz_body}"))?;
        let http01 = authz
            .challenges
            .iter()
            .find(|c| c.kind == "http-01")
            .ok_or_else(|| "ACME authorization missing http-01 challenge".to_string())?;

        if !valid_challenge_token(&http01.token) {
            return Err("ACME challenge token contains invalid characters".to_string());
        }
        fs::create_dir_all(challenge_root)
            .map_err(|e| format!("failed to prepare ACME challenge directory: {e}"))?;
        let token_path = challenge_root.join(&http01.token);
        let key_auth = format!("{}.{}", http01.token, thumbprint);
        fs::write(&token_path, key_auth.as_bytes())
            .map_err(|e| format!("failed to write ACME HTTP-01 challenge file: {e}"))?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let _ = fs::set_permissions(&token_path, fs::Permissions::from_mode(0o644));
        }
        _guards.push(ChallengeFileGuard { path: token_path });

        let (_, next_nonce) = post_jws(
            &client,
            &http01.url,
            &acct_key,
            Some(&kid),
            None,
            &nonce,
            Some(&json!({})),
        )
        .await?;
        nonce = next_nonce;
    }

    // Poll order until ready or valid
    for _ in 0..30 {
        if order.status == "ready" || order.status == "valid" {
            break;
        }
        if order.status == "invalid" {
            return Err("ACME order status became invalid".to_string());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let (poll_resp, next_nonce) = post_jws(
            &client,
            &order_url,
            &acct_key,
            Some(&kid),
            None,
            &nonce,
            None,
        )
        .await?;
        nonce = next_nonce;
        order = poll_resp
            .json()
            .await
            .map_err(|e| format!("invalid ACME order poll JSON: {e}"))?;
    }
    if order.status != "ready" && order.status != "valid" {
        return Err(format!(
            "ACME order did not become ready (status={})",
            order.status
        ));
    }

    let (csr_der, cert_key_pem) = generate_p256_csr_and_key_pem(domain, &rng)?;

    if order.status == "ready" {
        let finalize_body = json!({ "csr": URL_SAFE_NO_PAD.encode(&csr_der) });
        let (fin_resp, next_nonce) = post_jws(
            &client,
            &order.finalize,
            &acct_key,
            Some(&kid),
            None,
            &nonce,
            Some(&finalize_body),
        )
        .await?;
        nonce = next_nonce;
        order = fin_resp
            .json()
            .await
            .map_err(|e| format!("invalid ACME finalize JSON: {e}"))?;
    }

    for _ in 0..30 {
        if order.status == "valid" && order.certificate.is_some() {
            break;
        }
        if order.status == "invalid" {
            return Err("ACME order failed during finalization".to_string());
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
        let (poll_resp, next_nonce) = post_jws(
            &client,
            &order_url,
            &acct_key,
            Some(&kid),
            None,
            &nonce,
            None,
        )
        .await?;
        nonce = next_nonce;
        order = poll_resp
            .json()
            .await
            .map_err(|e| format!("invalid ACME order poll JSON: {e}"))?;
    }

    let cert_url = order
        .certificate
        .ok_or_else(|| "ACME order valid but certificate URL missing".to_string())?;
    let (cert_resp, _) = post_jws(
        &client,
        &cert_url,
        &acct_key,
        Some(&kid),
        None,
        &nonce,
        None,
    )
    .await?;
    let fullchain_pem = cert_resp
        .text()
        .await
        .map_err(|e| format!("failed to read issued certificate PEM: {e}"))?;
    if !fullchain_pem.contains("-----BEGIN CERTIFICATE-----") {
        return Err("ACME certificate response did not contain PEM".to_string());
    }
    Ok((fullchain_pem, cert_key_pem))
}

fn valid_challenge_token(token: &str) -> bool {
    !token.is_empty()
        && token.len() <= 128
        && token
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

async fn fetch_nonce(client: &Client, new_nonce_url: &str) -> Result<String, String> {
    let resp = client
        .head(new_nonce_url)
        .send()
        .await
        .map_err(|e| format!("ACME newNonce request failed: {e}"))?;
    resp.headers()
        .get("replay-nonce")
        .and_then(|v| v.to_str().ok())
        .map(str::to_string)
        .ok_or_else(|| "ACME newNonce response missing Replay-Nonce".to_string())
}

async fn post_jws(
    client: &Client,
    url: &str,
    key: &EcdsaKeyPair,
    kid: Option<&str>,
    jwk: Option<&Value>,
    nonce: &str,
    payload: Option<&Value>,
) -> Result<(reqwest::Response, String), String> {
    let mut current_nonce = nonce.to_string();
    for attempt in 0..3 {
        let mut protected = json!({
            "alg": "ES256",
            "nonce": current_nonce,
            "url": url,
        });
        if let Some(kid) = kid {
            protected["kid"] = Value::String(kid.to_string());
        } else if let Some(jwk) = jwk {
            protected["jwk"] = jwk.clone();
        }

        let protected_b64 = URL_SAFE_NO_PAD.encode(protected.to_string().as_bytes());
        let payload_b64 = match payload {
            Some(value) => URL_SAFE_NO_PAD.encode(value.to_string().as_bytes()),
            None => String::new(),
        };
        let signing_input = format!("{protected_b64}.{payload_b64}");
        let rng = SystemRandom::new();
        let sig = key
            .sign(&rng, signing_input.as_bytes())
            .map_err(|_| "failed to sign ACME JWS payload".to_string())?;
        let body = json!({
            "protected": protected_b64,
            "payload": payload_b64,
            "signature": URL_SAFE_NO_PAD.encode(sig.as_ref()),
        });

        let resp = client
            .post(url)
            .header("Content-Type", "application/jose+json")
            .json(&body)
            .send()
            .await
            .map_err(|e| format!("ACME POST {url} failed: {e}"))?;

        let next_nonce = resp
            .headers()
            .get("replay-nonce")
            .and_then(|v| v.to_str().ok())
            .map(str::to_string)
            .unwrap_or_default();

        if resp.status() == StatusCode::BAD_REQUEST || !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            if status == StatusCode::BAD_REQUEST
                && text.contains("badNonce")
                && !next_nonce.is_empty()
                && attempt < 2
            {
                current_nonce = next_nonce;
                continue;
            }
            return Err(format!("ACME {url} returned {status}: {text}"));
        }

        return Ok((resp, next_nonce));
    }
    Err(format!("ACME POST {url} exhausted badNonce retries"))
}

fn ec_jwk(key: &EcdsaKeyPair) -> Result<Value, String> {
    let pub_bytes = key.public_key().as_ref();
    if pub_bytes.len() != 65 || pub_bytes[0] != 0x04 {
        return Err("unexpected P-256 public key encoding".to_string());
    }
    let x = URL_SAFE_NO_PAD.encode(&pub_bytes[1..33]);
    let y = URL_SAFE_NO_PAD.encode(&pub_bytes[33..65]);
    Ok(json!({
        "crv": "P-256",
        "kty": "EC",
        "x": x,
        "y": y,
    }))
}

fn jwk_thumbprint(jwk: &Value) -> Result<String, String> {
    let canonical = json!({
        "crv": jwk["crv"],
        "kty": jwk["kty"],
        "x": jwk["x"],
        "y": jwk["y"],
    })
    .to_string();
    let hash = digest(&SHA256, canonical.as_bytes());
    Ok(URL_SAFE_NO_PAD.encode(hash.as_ref()))
}

pub fn generate_p256_csr_and_key_pem(
    domain: &str,
    rng: &SystemRandom,
) -> Result<(Vec<u8>, String), String> {
    let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, rng)
        .map_err(|_| "failed to generate certificate P-256 keypair".to_string())?;
    let keypair = EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_ASN1_SIGNING, pkcs8.as_ref(), rng)
        .map_err(|_| "failed to parse certificate P-256 keypair".to_string())?;
    let pub_bytes = keypair.public_key().as_ref();

    // Subject: CN=<domain>
    // OID 2.5.4.3 (commonName) = 06 03 55 04 03
    let cn_attr = der_tlv(
        0x30,
        &[der_oid(&[0x55, 0x04, 0x03]), der_utf8(domain)].concat(),
    );
    let rdn_set = der_tlv(0x31, &cn_attr);
    let subject = der_tlv(0x30, &rdn_set);

    // SubjectPublicKeyInfo for EC P-256:
    // alg: SEQUENCE { OID id-ecPublicKey (1.2.840.10045.2.1), OID prime256v1 (1.2.840.10045.3.1.7) }
    let ec_alg = der_tlv(
        0x30,
        &[
            der_oid(&[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x02, 0x01]),
            der_oid(&[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x03, 0x01, 0x07]),
        ]
        .concat(),
    );
    let spki = der_tlv(0x30, &[ec_alg, der_bit_string(pub_bytes)].concat());

    // extensionRequest (1.2.840.113549.1.9.14) with subjectAltName (2.5.29.17) dNSName
    let dns_name = der_tlv(0x82, domain.as_bytes());
    let san_seq = der_tlv(0x30, &dns_name);
    let san_ext = der_tlv(
        0x30,
        &[der_oid(&[0x55, 0x1d, 0x11]), der_tlv(0x04, &san_seq)].concat(),
    );
    let exts_seq = der_tlv(0x30, &san_ext);
    let ext_req_attr = der_tlv(
        0x30,
        &[
            der_oid(&[0x2a, 0x86, 0x48, 0x86, 0xf7, 0x0d, 0x01, 0x09, 0x0e]),
            der_tlv(0x31, &exts_seq),
        ]
        .concat(),
    );
    let attrs = der_tlv(0xa0, &ext_req_attr);

    // CertificationRequestInfo
    let cri = der_tlv(
        0x30,
        &[vec![0x02, 0x01, 0x00], subject, spki, attrs].concat(),
    );
    let sig = keypair
        .sign(rng, &cri)
        .map_err(|_| "failed to sign PKCS#10 CSR".to_string())?;

    // ecdsa-with-SHA256: 1.2.840.10045.4.3.2
    let sig_alg = der_tlv(
        0x30,
        &der_oid(&[0x2a, 0x86, 0x48, 0xce, 0x3d, 0x04, 0x03, 0x02]),
    );
    let csr_der = der_tlv(0x30, &[cri, sig_alg, der_bit_string(sig.as_ref())].concat());
    let key_pem = pem_encode("PRIVATE KEY", pkcs8.as_ref());

    Ok((csr_der, key_pem))
}

fn der_tlv(tag: u8, content: &[u8]) -> Vec<u8> {
    let mut out = vec![tag];
    let len = content.len();
    if len < 128 {
        out.push(len as u8);
    } else if len <= 0xff {
        out.push(0x81);
        out.push(len as u8);
    } else {
        out.push(0x82);
        out.push((len >> 8) as u8);
        out.push(len as u8);
    }
    out.extend_from_slice(content);
    out
}

fn der_oid(bytes: &[u8]) -> Vec<u8> {
    der_tlv(0x06, bytes)
}

fn der_utf8(s: &str) -> Vec<u8> {
    der_tlv(0x0c, s.as_bytes())
}

fn der_bit_string(bytes: &[u8]) -> Vec<u8> {
    let mut content = Vec::with_capacity(bytes.len() + 1);
    content.push(0x00);
    content.extend_from_slice(bytes);
    der_tlv(0x03, &content)
}

fn pem_encode(label: &str, der: &[u8]) -> String {
    let b64 = STANDARD.encode(der);
    let mut out = format!("-----BEGIN {label}-----\n");
    for chunk in b64.as_bytes().chunks(64) {
        if let Ok(line) = std::str::from_utf8(chunk) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out.push_str(&format!("-----END {label}-----\n"));
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;

    #[test]
    fn generates_valid_pkcs10_csr_and_private_key() {
        let rng = SystemRandom::new();
        let (csr_der, key_pem) = generate_p256_csr_and_key_pem("app.sprint4.test", &rng).unwrap();
        assert!(key_pem.contains("-----BEGIN PRIVATE KEY-----"));
        assert!(key_pem.contains("-----END PRIVATE KEY-----"));
        assert!(!csr_der.is_empty());

        if Command::new("openssl").arg("version").output().is_ok() {
            let dir = tempfile::tempdir().unwrap();
            let csr_path = dir.path().join("req.der");
            fs::write(&csr_path, &csr_der).unwrap();
            let output = Command::new("openssl")
                .args(["req", "-inform", "DER", "-in"])
                .arg(&csr_path)
                .args(["-verify", "-noout"])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "openssl verify failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
    }
}

#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use hosting::domain::DomainName;
use rcgen::{CertificateParams, DnType, KeyPair};
use ring::digest::{self, Context};
use ring::rand::SystemRandom;
use ring::signature::{self, EcdsaKeyPair, KeyPair as _};
use serde_json::json;

#[derive(Debug)]
pub enum AcmeError {
    Io(io::Error),
    Network(String),
    Protocol(String),
    Crypto(String),
}

impl From<io::Error> for AcmeError {
    fn from(err: io::Error) -> Self {
        AcmeError::Io(err)
    }
}

impl PartialEq for AcmeError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Io(_), Self::Io(_)) => true,
            (Self::Network(a), Self::Network(b)) => a == b,
            (Self::Protocol(a), Self::Protocol(b)) => a == b,
            (Self::Crypto(a), Self::Crypto(b)) => a == b,
            _ => false,
        }
    }
}

pub type AcmeResponse = (u16, Vec<(String, String)>, String);

pub trait AcmeTransport {
    fn request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<AcmeResponse, AcmeError>;
}

pub struct AcmeClient<T: AcmeTransport> {
    pub directory_url: String,
    pub transport: T,
    pub nonce: Option<String>,
    pub account_url: Option<String>,
    pub key_pair: signature::EcdsaKeyPair,
}

impl<T: AcmeTransport> AcmeClient<T> {
    pub fn new(
        directory_url: String,
        transport: T,
        key_pair_pkcs8: &[u8],
    ) -> Result<Self, AcmeError> {
        let key_pair = EcdsaKeyPair::from_pkcs8(
            &signature::ECDSA_P256_SHA256_FIXED_SIGNING,
            key_pair_pkcs8,
            &SystemRandom::new(),
        )
        .map_err(|_| AcmeError::Crypto("Invalid key pair".into()))?;

        Ok(Self {
            directory_url,
            transport,
            nonce: None,
            account_url: None,
            key_pair,
        })
    }

    fn jwk(&self) -> serde_json::Value {
        let pub_key = self.key_pair.public_key().as_ref();
        // ring's uncompressed SEC1 starts with 0x04, followed by 32 bytes X and 32 bytes Y
        let x = URL_SAFE_NO_PAD.encode(&pub_key[1..33]);
        let y = URL_SAFE_NO_PAD.encode(&pub_key[33..65]);

        json!({
            "crv": "P-256",
            "kty": "EC",
            "x": x,
            "y": y
        })
    }

    fn account_thumbprint(&self) -> Result<String, AcmeError> {
        let jwk = self.jwk();
        let jwk_str = serde_json::to_string(&jwk).unwrap();
        let mut ctx = Context::new(&digest::SHA256);
        ctx.update(jwk_str.as_bytes());
        let digest = ctx.finish();
        Ok(URL_SAFE_NO_PAD.encode(digest.as_ref()))
    }

    fn sign_jws(
        &self,
        nonce: &str,
        url: &str,
        payload: &str,
        kid: Option<&str>,
    ) -> Result<String, AcmeError> {
        let b64_payload = if payload.is_empty() {
            "".to_string()
        } else {
            URL_SAFE_NO_PAD.encode(payload)
        };

        let mut protected_header = json!({
            "alg": "ES256",
            "nonce": nonce,
            "url": url,
        });

        if let Some(k) = kid {
            protected_header
                .as_object_mut()
                .unwrap()
                .insert("kid".to_string(), json!(k));
        } else {
            protected_header
                .as_object_mut()
                .unwrap()
                .insert("jwk".to_string(), self.jwk());
        }

        let b64_protected =
            URL_SAFE_NO_PAD.encode(serde_json::to_string(&protected_header).unwrap());
        let signing_input = format!("{}.{}", b64_protected, b64_payload);

        let rng = SystemRandom::new();
        let signature = self
            .key_pair
            .sign(&rng, signing_input.as_bytes())
            .map_err(|_| AcmeError::Crypto("Signing failed".into()))?;

        let b64_signature = URL_SAFE_NO_PAD.encode(signature.as_ref());

        let jws = json!({
            "protected": b64_protected,
            "payload": b64_payload,
            "signature": b64_signature
        });

        Ok(serde_json::to_string(&jws).unwrap())
    }

    pub fn fetch_nonce(&mut self) -> Result<(), AcmeError> {
        let (directory_status, _, body) =
            self.transport
                .request("GET", &self.directory_url, &[], None)?;
        if directory_status != 200 {
            return Err(AcmeError::Protocol(format!(
                "Directory request failed with status: {directory_status}"
            )));
        }
        let new_nonce_url = Self::extract_json_string(&body, "newNonce").ok_or_else(|| {
            AcmeError::Protocol("newNonce endpoint not found in directory".into())
        })?;

        let (status, headers, _) = self.transport.request("HEAD", &new_nonce_url, &[], None)?;
        if status != 200 && status != 204 {
            return Err(AcmeError::Protocol(format!(
                "Failed to fetch nonce, status: {}",
                status
            )));
        }

        self.extract_nonce_from_response(&headers)?;
        Ok(())
    }

    fn extract_nonce_from_response(
        &mut self,
        headers: &[(String, String)],
    ) -> Result<(), AcmeError> {
        let nonce = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("replay-nonce"))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| AcmeError::Protocol("Replay-Nonce header missing".into()))?;
        self.nonce = Some(nonce);
        Ok(())
    }

    fn update_nonce_from_response(&mut self, headers: &[(String, String)]) {
        // The request nonce was already consumed by `get_nonce`. If the
        // response omits Replay-Nonce, leave the cache empty so the next
        // request obtains a fresh nonce instead of reusing the old one.
        self.nonce = headers
            .iter()
            .find(|(key, _)| key.eq_ignore_ascii_case("replay-nonce"))
            .map(|(_, value)| value.clone());
    }

    pub fn get_nonce(&mut self) -> Result<String, AcmeError> {
        if self.nonce.is_none() {
            self.fetch_nonce()?;
        }
        self.nonce
            .take()
            .ok_or_else(|| AcmeError::Protocol("No nonce available after fetch".into()))
    }

    pub fn create_account(&mut self, email: &str) -> Result<String, AcmeError> {
        let (directory_status, _, body) =
            self.transport
                .request("GET", &self.directory_url, &[], None)?;
        if directory_status != 200 {
            return Err(AcmeError::Protocol(format!(
                "Directory request failed with status: {directory_status}"
            )));
        }
        let new_account_url = Self::extract_json_string(&body, "newAccount").ok_or_else(|| {
            AcmeError::Protocol("newAccount endpoint not found in directory".into())
        })?;

        let nonce = self.get_nonce()?;

        let payload = json!({
            "termsOfServiceAgreed": true,
            "contact": [format!("mailto:{}", email)]
        })
        .to_string();

        let jws = self.sign_jws(&nonce, &new_account_url, &payload, None)?;

        let (status, headers, _) = self.transport.request(
            "POST",
            &new_account_url,
            &[("Content-Type", "application/jose+json")],
            Some(&jws),
        )?;

        self.update_nonce_from_response(&headers);
        if status != 201 && status != 200 {
            return Err(AcmeError::Protocol(format!(
                "Account creation failed with status: {}",
                status
            )));
        }

        let account_url = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("location"))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                AcmeError::Protocol("Location header missing in account response".into())
            })?;

        self.account_url = Some(account_url.clone());
        Ok(account_url)
    }

    pub fn order_certificate(&mut self, domain: &str) -> Result<String, AcmeError> {
        DomainName::new(domain.to_string())
            .map_err(|_| AcmeError::Protocol("Invalid or unsupported domain name".into()))?;
        if self.account_url.is_none() {
            return Err(AcmeError::Protocol("No account URL available".into()));
        }

        let (directory_status, _, body) =
            self.transport
                .request("GET", &self.directory_url, &[], None)?;
        if directory_status != 200 {
            return Err(AcmeError::Protocol(format!(
                "Directory request failed with status: {directory_status}"
            )));
        }
        let new_order_url = Self::extract_json_string(&body, "newOrder").ok_or_else(|| {
            AcmeError::Protocol("newOrder endpoint not found in directory".into())
        })?;

        let nonce = self.get_nonce()?;

        let payload = json!({
            "identifiers": [{"type": "dns", "value": domain}]
        })
        .to_string();

        let jws = self.sign_jws(
            &nonce,
            &new_order_url,
            &payload,
            self.account_url.as_deref(),
        )?;

        let (status, headers, _) = self.transport.request(
            "POST",
            &new_order_url,
            &[("Content-Type", "application/jose+json")],
            Some(&jws),
        )?;

        self.update_nonce_from_response(&headers);
        if status != 201 {
            return Err(AcmeError::Protocol(format!(
                "Order creation failed with status: {}",
                status
            )));
        }

        let order_url = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("location"))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                AcmeError::Protocol("Location header missing in order response".into())
            })?;

        Ok(order_url)
    }

    pub fn finalize_order(
        &mut self,
        finalize_url: &str,
        domain: &str,
        cert_key_pkcs8: &[u8],
    ) -> Result<String, AcmeError> {
        DomainName::new(domain.to_string())
            .map_err(|_| AcmeError::Protocol("Invalid or unsupported domain name".into()))?;
        if self.account_url.is_none() {
            return Err(AcmeError::Protocol("No account URL available".into()));
        }

        let nonce = self.get_nonce()?;

        let key_pair = KeyPair::from_der(cert_key_pkcs8)
            .map_err(|_| AcmeError::Crypto("Invalid certificate key".into()))?;
        let mut params = CertificateParams::new(vec![domain.to_string()]);
        params.distinguished_name.push(DnType::CommonName, domain);
        params.key_pair = Some(key_pair);
        let cert = rcgen::Certificate::from_params(params)
            .map_err(|_| AcmeError::Crypto("Failed to create certificate params".into()))?;
        let csr = cert
            .serialize_request_der()
            .map_err(|_| AcmeError::Crypto("Failed to serialize CSR".into()))?;
        let csr_b64 = URL_SAFE_NO_PAD.encode(csr);

        let payload = json!({ "csr": csr_b64 }).to_string();
        let jws = self.sign_jws(&nonce, finalize_url, &payload, self.account_url.as_deref())?;

        let (status, headers, body) = self.transport.request(
            "POST",
            finalize_url,
            &[("Content-Type", "application/jose+json")],
            Some(&jws),
        )?;

        self.update_nonce_from_response(&headers);
        if status != 200 {
            return Err(AcmeError::Protocol(format!("Finalize failed: {}", status)));
        }

        let cert_url = Self::extract_json_string(&body, "certificate").ok_or_else(|| {
            AcmeError::Protocol("certificate url missing in finalize response".into())
        })?;

        Ok(cert_url)
    }

    pub fn download_certificate(&mut self, cert_url: &str) -> Result<String, AcmeError> {
        if self.account_url.is_none() {
            return Err(AcmeError::Protocol("No account URL available".into()));
        }

        let nonce = self.get_nonce()?;

        let jws = self.sign_jws(&nonce, cert_url, "", self.account_url.as_deref())?;

        let (status, headers, body) = self.transport.request(
            "POST",
            cert_url,
            &[("Content-Type", "application/jose+json")],
            Some(&jws),
        )?;

        self.update_nonce_from_response(&headers);
        if status != 200 {
            return Err(AcmeError::Protocol(format!("Download failed: {}", status)));
        }

        Ok(body)
    }

    pub fn get_challenge_auth(&self, token: &str) -> Result<String, AcmeError> {
        let thumbprint = self.account_thumbprint()?;
        Ok(format!("{}.{}", token, thumbprint))
    }

    pub fn complete_challenge(&mut self, challenge_url: &str) -> Result<(), AcmeError> {
        if self.account_url.is_none() {
            return Err(AcmeError::Protocol("No account URL available".into()));
        }

        let nonce = self.get_nonce()?;

        let payload = json!({}).to_string();
        let jws = self.sign_jws(&nonce, challenge_url, &payload, self.account_url.as_deref())?;

        let (status, headers, _) = self.transport.request(
            "POST",
            challenge_url,
            &[("Content-Type", "application/jose+json")],
            Some(&jws),
        )?;

        self.update_nonce_from_response(&headers);
        if status != 200 {
            return Err(AcmeError::Protocol(format!(
                "Challenge complete failed: {}",
                status
            )));
        }

        Ok(())
    }

    pub fn host_challenge(&self, webroot: &Path, token: &str) -> Result<(), AcmeError> {
        // ACME tokens are base64url components, never filesystem paths.
        if token.is_empty()
            || token.len() > 256
            || !token
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
        {
            return Err(AcmeError::Protocol("Invalid challenge token".into()));
        }

        let challenge_dir = webroot.join(".well-known").join("acme-challenge");
        fs::create_dir_all(&challenge_dir)?;
        let key_auth = self.get_challenge_auth(token)?;

        let challenge_file = challenge_dir.join(token);

        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o644)
            .open(&challenge_file)?;

        file.write_all(key_auth.as_bytes())?;
        Ok(())
    }

    pub fn manage_account_key(&self, account_dir: &Path, key_pem: &str) -> Result<(), AcmeError> {
        fs::create_dir_all(account_dir)?;
        let key_path = account_dir.join("account.key");

        let mut key_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&key_path)?;
        key_file.write_all(key_pem.as_bytes())?;

        Ok(())
    }

    pub fn persist_certs(
        &self,
        dest_dir: &Path,
        cert_pem: &str,
        priv_key_pem: &str,
    ) -> Result<(), AcmeError> {
        fs::create_dir_all(dest_dir)?;

        let cert_path = dest_dir.join("cert.pem");
        let key_path = dest_dir.join("privkey.pem");

        let mut cert_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&cert_path)?;
        cert_file.write_all(cert_pem.as_bytes())?;

        let mut key_file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&key_path)?;
        key_file.write_all(priv_key_pem.as_bytes())?;

        Ok(())
    }

    fn extract_json_string(json: &str, key: &str) -> Option<String> {
        let search = format!("\"{}\"", key);
        let mut start_idx = json.find(&search)?;
        start_idx += search.len();

        let colon_idx = json[start_idx..].find(':')?;
        start_idx += colon_idx + 1;

        let quote_start_idx = json[start_idx..].find('"')?;
        start_idx += quote_start_idx + 1;

        let quote_end_idx = json[start_idx..].find('"')?;
        Some(json[start_idx..start_idx + quote_end_idx].to_string())
    }
}

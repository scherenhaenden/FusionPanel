#![forbid(unsafe_code)]

use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

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

pub trait AcmeTransport {
    fn request(
        &self,
        method: &str,
        url: &str,
        headers: &[(&str, &str)],
        body: Option<&str>,
    ) -> Result<(u16, Vec<(String, String)>, String), AcmeError>;
}

pub struct AcmeClient<T: AcmeTransport> {
    pub directory_url: String,
    pub transport: T,
    pub nonce: Option<String>,
}

impl<T: AcmeTransport> AcmeClient<T> {
    pub fn new(directory_url: String, transport: T) -> Self {
        Self {
            directory_url,
            transport,
            nonce: None,
        }
    }

    pub fn fetch_nonce(&mut self) -> Result<(), AcmeError> {
        let (_, _, body) = self
            .transport
            .request("GET", &self.directory_url, &[], None)?;
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

    pub fn get_nonce(&mut self) -> Result<String, AcmeError> {
        if self.nonce.is_none() {
            self.fetch_nonce()?;
        }
        self.nonce
            .take()
            .ok_or_else(|| AcmeError::Protocol("No nonce available after fetch".into()))
    }

    pub fn create_account(&mut self, email: &str) -> Result<String, AcmeError> {
        let (_, _, body) = self
            .transport
            .request("GET", &self.directory_url, &[], None)?;
        let new_account_url = Self::extract_json_string(&body, "newAccount").ok_or_else(|| {
            AcmeError::Protocol("newAccount endpoint not found in directory".into())
        })?;

        let nonce = self.get_nonce()?;

        let payload = format!(
            r#"{{"termsOfServiceAgreed":true,"contact":["mailto:{}"]}}"#,
            email
        );

        let (status, headers, _) = self.transport.request(
            "POST",
            &new_account_url,
            &[
                ("Content-Type", "application/jose+json"),
                ("X-Nonce", &nonce),
            ],
            Some(&payload),
        )?;

        if status != 201 && status != 200 {
            return Err(AcmeError::Protocol(format!(
                "Account creation failed with status: {}",
                status
            )));
        }

        self.extract_nonce_from_response(&headers).ok();

        let account_url = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("location"))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                AcmeError::Protocol("Location header missing in account response".into())
            })?;

        Ok(account_url)
    }

    pub fn order_certificate(&mut self, domain: &str) -> Result<String, AcmeError> {
        let (_, _, body) = self
            .transport
            .request("GET", &self.directory_url, &[], None)?;
        let new_order_url = Self::extract_json_string(&body, "newOrder").ok_or_else(|| {
            AcmeError::Protocol("newOrder endpoint not found in directory".into())
        })?;

        let nonce = self.get_nonce()?;

        // In a real client, body is a JWS. Here we simulate the payload.
        let payload = format!(
            r#"{{"identifiers":[{{"type":"dns","value":"{}"}}]}}"#,
            domain
        );

        let (status, headers, _) = self.transport.request(
            "POST",
            &new_order_url,
            &[
                ("Content-Type", "application/jose+json"),
                ("X-Nonce", &nonce),
            ],
            Some(&payload),
        )?;

        if status != 201 {
            return Err(AcmeError::Protocol(format!(
                "Order creation failed with status: {}",
                status
            )));
        }

        self.extract_nonce_from_response(&headers).ok(); // Optimistically update nonce

        let order_url = headers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("location"))
            .map(|(_, v)| v.clone())
            .ok_or_else(|| {
                AcmeError::Protocol("Location header missing in order response".into())
            })?;

        Ok(order_url)
    }

    pub fn host_challenge(
        &self,
        webroot: &Path,
        token: &str,
        key_auth: &str,
    ) -> Result<(), AcmeError> {
        let challenge_dir = webroot.join(".well-known").join("acme-challenge");
        fs::create_dir_all(&challenge_dir)?;

        // Defend against path traversal in token
        if token.contains('/') || token.contains('\\') || token.contains("..") {
            return Err(AcmeError::Protocol("Invalid challenge token".into()));
        }

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

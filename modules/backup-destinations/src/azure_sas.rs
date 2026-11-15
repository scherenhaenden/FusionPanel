#![forbid(unsafe_code)]

use hmac::{Hmac, Mac};
use sha2::Sha256;
use urlencoding::encode;
use thiserror::Error;

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Error)]
pub enum AzureSasError {
    #[error("Invalid base64 in account key")]
    InvalidBase64,
    #[error("HMAC error")]
    HmacError,
}

/// A builder for Azure Blob Storage Shared Access Signature (SAS) tokens.
pub struct AzureSasBuilder {
    account_name: String,
    account_key: String,
    container_name: String,
    permissions: String,
    start_time: String,
    expiry_time: String,
    protocol: String,
    version: String,
}

impl AzureSasBuilder {
    pub fn new(account_name: &str, account_key: &str, container_name: &str) -> Self {
        Self {
            account_name: account_name.to_string(),
            account_key: account_key.to_string(),
            container_name: container_name.to_string(),
            permissions: "rwd".to_string(), // read, write, delete
            start_time: "".to_string(),
            expiry_time: "".to_string(),
            protocol: "https".to_string(),
            version: "2020-02-10".to_string(),
        }
    }

    pub fn with_permissions(mut self, permissions: &str) -> Self {
        self.permissions = permissions.to_string();
        self
    }

    pub fn with_start_time(mut self, start_time: &str) -> Self {
        self.start_time = start_time.to_string();
        self
    }

    pub fn with_expiry_time(mut self, expiry_time: &str) -> Self {
        self.expiry_time = expiry_time.to_string();
        self
    }

    pub fn with_protocol(mut self, protocol: &str) -> Self {
        self.protocol = protocol.to_string();
        self
    }

    pub fn with_version(mut self, version: &str) -> Self {
        self.version = version.to_string();
        self
    }

    pub fn generate_sas_token(&self) -> Result<String, AzureSasError> {
        let key = decode_b64(&self.account_key).ok_or(AzureSasError::InvalidBase64)?;

        let canonicalized_resource = format!("/blob/{}/{}", self.account_name, self.container_name);

        // For versions prior to 2020-12-06, there are 15 newline-separated fields (no signedEncryptionScope)
        let string_to_sign = format!(
            "{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}\n{}",
            self.permissions,
            self.start_time,
            self.expiry_time,
            canonicalized_resource,
            "", // signedIdentifier
            "", // signedIP
            self.protocol, // signedProtocol
            self.version, // signedVersion
            "c", // signedResource (container)
            "", // signedSnapshotTime
            // "", // signedEncryptionScope - NOT included for 2020-02-10
            "", // rscc
            "", // rscd
            "", // rsce
            "", // rscl
            ""  // rsct
        );

        let mut mac = HmacSha256::new_from_slice(&key).map_err(|_| AzureSasError::HmacError)?;
        mac.update(string_to_sign.as_bytes());
        let result = mac.finalize();
        let sig = encode_b64(&result.into_bytes());

        // Construct SAS token query string
        let mut qs = Vec::new();
        if !self.start_time.is_empty() {
            qs.push(format!("st={}", encode(&self.start_time)));
        }
        if !self.expiry_time.is_empty() {
            qs.push(format!("se={}", encode(&self.expiry_time)));
        }
        qs.push(format!("sp={}", self.permissions));
        qs.push(format!("sv={}", self.version));
        qs.push("sr=c".to_string());
        qs.push(format!("spr={}", self.protocol));
        qs.push(format!("sig={}", encode(&sig)));

        Ok(qs.join("&"))
    }
}

pub fn decode_b64(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    let mut val = 0u32;
    let mut bits = 0;

    for c in input.chars() {
        if c == '=' { break; }
        if c.is_whitespace() { continue; }

        let b = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };

        val = (val << 6) | b;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((val >> bits) as u8);
        }
    }
    Some(out)
}

pub fn encode_b64(input: &[u8]) -> String {
    let b64 = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();
    let mut val = 0u32;
    let mut bits = 0;

    for &b in input {
        val = (val << 8) | (b as u32);
        bits += 8;

        while bits >= 6 {
            bits -= 6;
            out.push(b64[((val >> bits) & 0x3F) as usize] as char);
        }
    }

    if bits > 0 {
        out.push(b64[((val << (6 - bits)) & 0x3F) as usize] as char);
    }

    while out.len() % 4 != 0 {
        out.push('=');
    }

    out
}

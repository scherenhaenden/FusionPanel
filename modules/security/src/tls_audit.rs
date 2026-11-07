#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum TlsProtocol {
    #[serde(rename = "TLSv1.0")]
    Tls1_0,
    #[serde(rename = "TLSv1.1")]
    Tls1_1,
    #[serde(rename = "TLSv1.2")]
    Tls1_2,
    #[serde(rename = "TLSv1.3")]
    Tls1_3,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsConfig {
    pub protocols: Vec<TlsProtocol>,
    pub ciphers: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TlsAuditResult {
    pub is_modern: bool,
    pub is_intermediate: bool,
    pub has_weak_ciphers: bool,
    pub weak_ciphers: Vec<String>,
    pub deprecated_protocols: Vec<TlsProtocol>,
}

pub const MODERN_CIPHERS: &[&str] = &[
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
];

pub const INTERMEDIATE_CIPHERS: &[&str] = &[
    "TLS_AES_128_GCM_SHA256",
    "TLS_AES_256_GCM_SHA384",
    "TLS_CHACHA20_POLY1305_SHA256",
    "ECDHE-ECDSA-AES128-GCM-SHA256",
    "ECDHE-RSA-AES128-GCM-SHA256",
    "ECDHE-ECDSA-AES256-GCM-SHA384",
    "ECDHE-RSA-AES256-GCM-SHA384",
    "ECDHE-ECDSA-CHACHA20-POLY1305",
    "ECDHE-RSA-CHACHA20-POLY1305",
    "DHE-RSA-AES128-GCM-SHA256",
    "DHE-RSA-AES256-GCM-SHA384",
];

impl TlsConfig {
    pub fn new(protocols: Vec<TlsProtocol>, ciphers: Vec<String>) -> Self {
        Self { protocols, ciphers }
    }

    pub fn audit(&self) -> TlsAuditResult {
        let mut weak_ciphers = Vec::new();
        for cipher in &self.ciphers {
            let upper = cipher.to_uppercase();
            if upper.contains("CBC") || upper.contains("3DES") || upper.contains("RC4") || upper.contains("DES") || upper.contains("MD5") {
                weak_ciphers.push(cipher.clone());
            }
        }

        let mut deprecated_protocols = Vec::new();
        for proto in &self.protocols {
            match proto {
                TlsProtocol::Tls1_0 | TlsProtocol::Tls1_1 => {
                    deprecated_protocols.push(proto.clone());
                }
                _ => {}
            }
        }

        deprecated_protocols.sort_by(|a, b| {
            let a_val = match a { TlsProtocol::Tls1_0 => 1, TlsProtocol::Tls1_1 => 2, TlsProtocol::Tls1_2 => 3, TlsProtocol::Tls1_3 => 4 };
            let b_val = match b { TlsProtocol::Tls1_0 => 1, TlsProtocol::Tls1_1 => 2, TlsProtocol::Tls1_2 => 3, TlsProtocol::Tls1_3 => 4 };
            a_val.cmp(&b_val)
        });
        deprecated_protocols.dedup();

        let is_modern_proto = self.protocols.len() == 1 && self.protocols.contains(&TlsProtocol::Tls1_3);
        let mut is_modern_cipher = true;
        for cipher in &self.ciphers {
            if !MODERN_CIPHERS.contains(&cipher.as_str()) {
                is_modern_cipher = false;
                break;
            }
        }
        let is_modern = is_modern_proto && is_modern_cipher && !self.ciphers.is_empty();

        let is_intermediate_proto = !self.protocols.contains(&TlsProtocol::Tls1_0)
            && !self.protocols.contains(&TlsProtocol::Tls1_1)
            && (self.protocols.contains(&TlsProtocol::Tls1_2) || self.protocols.contains(&TlsProtocol::Tls1_3));

        let mut is_intermediate_cipher = true;
        for cipher in &self.ciphers {
            if !INTERMEDIATE_CIPHERS.contains(&cipher.as_str()) {
                is_intermediate_cipher = false;
                break;
            }
        }
        let is_intermediate = is_intermediate_proto && is_intermediate_cipher && !self.ciphers.is_empty() && !is_modern;

        TlsAuditResult {
            is_modern,
            is_intermediate,
            has_weak_ciphers: !weak_ciphers.is_empty(),
            weak_ciphers,
            deprecated_protocols,
        }
    }
}

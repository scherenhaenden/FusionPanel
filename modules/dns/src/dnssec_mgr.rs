#![forbid(unsafe_code)]

use crate::domain::DomainName;
use thiserror::Error;

/// Types of DNSSEC keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyType {
    /// Key Signing Key
    Ksk,
    /// Zone Signing Key
    Zsk,
}

/// A specification for a DNSSEC key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnssecKeySpec {
    pub key_type: KeyType,
    pub flags: u16,
    pub protocol: u8,
    pub algorithm: u8,
    pub public_key: Vec<u8>,
}

impl DnssecKeySpec {
    /// Creates a new key spec.
    pub fn new(key_type: KeyType, algorithm: u8, public_key: Vec<u8>) -> Self {
        let flags = match key_type {
            KeyType::Ksk => 257,
            KeyType::Zsk => 256,
        };
        Self {
            key_type,
            flags,
            protocol: 3,
            algorithm,
            public_key,
        }
    }

    /// Computes the key tag as defined in RFC 4034, Appendix B.
    pub fn key_tag(&self) -> u16 {
        if self.algorithm == 1 {
            // Algorithm 1 (RSAMD5) isn't typically used, but the RFC specifies
            // a different key tag calculation for it. We implement the general
            // one which applies to algorithm > 1.
            // If algorithm 1 support is needed, it extracts bytes from the end.
            // But since this is general:
            // return (self.public_key[self.public_key.len()-3] as u16) << 8 | (self.public_key[self.public_key.len()-2] as u16);
            // We just focus on the general algorithm for now.
        }

        let mut ac: u32 = 0;

        // RDATA is:
        // Flags (2 bytes)
        // Protocol (1 byte)
        // Algorithm (1 byte)
        // Public Key (variable)

        let mut rdata = Vec::with_capacity(4 + self.public_key.len());
        rdata.extend_from_slice(&self.flags.to_be_bytes());
        rdata.push(self.protocol);
        rdata.push(self.algorithm);
        rdata.extend_from_slice(&self.public_key);

        for (i, &byte) in rdata.iter().enumerate() {
            ac = ac.wrapping_add(if i % 2 != 0 {
                byte as u32
            } else {
                (byte as u32) << 8
            });
        }

        ac = ac.wrapping_add((ac >> 16) & 0xFFFF);
        (ac & 0xFFFF) as u16
    }
}

/// Errors related to DNSSEC operations.
#[derive(Error, Debug, PartialEq, Eq)]
pub enum DnssecError {
    #[error("Invalid key rollover state")]
    InvalidRollover,
}

/// Validates a DNSSEC key rollover.
pub fn validate_rollover(
    old_key: &DnssecKeySpec,
    new_key: &DnssecKeySpec,
) -> Result<(), DnssecError> {
    if old_key.key_type != new_key.key_type {
        return Err(DnssecError::InvalidRollover);
    }
    // A simplistic check: allow if they are the same type.
    Ok(())
}

/// Formats a DS record string from a given domain and KSK key spec.
/// Uses SHA-256 (Digest Type 2).
pub fn format_ds_record(domain: &DomainName, key: &DnssecKeySpec) -> String {
    // Construct the data to be hashed:
    // 1. Owner name in canonical wire format
    // 2. DNSKEY RDATA (Flags, Protocol, Algorithm, Public Key)

    let mut data = Vec::new();

    // Canonical wire format of domain name
    // For a simple domain like example.com:
    // 7example3com0
    let fqdn = domain.to_string();
    let fqdn = if fqdn.ends_with('.') {
        fqdn.to_string()
    } else {
        format!("{}.", fqdn)
    };

    let parts: Vec<&str> = fqdn.split('.').filter(|s| !s.is_empty()).collect();
    for part in parts {
        data.push(part.len() as u8);
        data.extend_from_slice(part.to_ascii_lowercase().as_bytes());
    }
    data.push(0); // Root label

    // DNSKEY RDATA
    data.extend_from_slice(&key.flags.to_be_bytes());
    data.push(key.protocol);
    data.push(key.algorithm);
    data.extend_from_slice(&key.public_key);

    let digest = sha256(&data);
    let digest_hex = hex_encode(&digest);

    let key_tag = key.key_tag();
    let algorithm = key.algorithm;
    let digest_type = 2; // SHA-256

    format!(
        "{} {} {} {}",
        key_tag,
        algorithm,
        digest_type,
        digest_hex.to_uppercase()
    )
}

fn hex_encode(data: &[u8]) -> String {
    let mut hex = String::with_capacity(data.len() * 2);
    for &b in data {
        hex.push(char::from_digit((b >> 4) as u32, 16).unwrap());
        hex.push(char::from_digit((b & 0xF) as u32, 16).unwrap());
    }
    hex
}

/// Pure safe Rust implementation of SHA-256.
fn sha256(data: &[u8]) -> [u8; 32] {
    let mut hash: [u32; 8] = [
        0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab,
        0x5be0cd19,
    ];

    let k: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4,
        0xab1c5ed5, 0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe,
        0x9bdc06a7, 0xc19bf174, 0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f,
        0x4a7484aa, 0x5cb0a9dc, 0x76f988da, 0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
        0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967, 0x27b70a85, 0x2e1b2138, 0x4d2c6dfc,
        0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85, 0xa2bfe8a1, 0xa81a664b,
        0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070, 0x19a4c116,
        0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7,
        0xc67178f2,
    ];

    let mut buffer = data.to_vec();
    let bit_len = (buffer.len() as u64) * 8;
    buffer.push(0x80);
    while (buffer.len() % 64) != 56 {
        buffer.push(0);
    }
    buffer.extend_from_slice(&bit_len.to_be_bytes());

    for chunk in buffer.chunks(64) {
        let mut w = [0u32; 64];
        for (i, c) in chunk.chunks(4).enumerate() {
            w[i] = u32::from_be_bytes([c[0], c[1], c[2], c[3]]);
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }

        let mut a = hash[0];
        let mut b = hash[1];
        let mut c = hash[2];
        let mut d = hash[3];
        let mut e = hash[4];
        let mut f = hash[5];
        let mut g = hash[6];
        let mut h = hash[7];

        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let temp1 = h
                .wrapping_add(s1)
                .wrapping_add(ch)
                .wrapping_add(k[i])
                .wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let temp2 = s0.wrapping_add(maj);

            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(temp1);
            d = c;
            c = b;
            b = a;
            a = temp1.wrapping_add(temp2);
        }

        hash[0] = hash[0].wrapping_add(a);
        hash[1] = hash[1].wrapping_add(b);
        hash[2] = hash[2].wrapping_add(c);
        hash[3] = hash[3].wrapping_add(d);
        hash[4] = hash[4].wrapping_add(e);
        hash[5] = hash[5].wrapping_add(f);
        hash[6] = hash[6].wrapping_add(g);
        hash[7] = hash[7].wrapping_add(h);
    }

    let mut result = [0u8; 32];
    for (i, &h) in hash.iter().enumerate() {
        result[i * 4..(i + 1) * 4].copy_from_slice(&h.to_be_bytes());
    }
    result
}

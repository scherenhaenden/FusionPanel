#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::net::IpAddr;
use std::str::FromStr;
use thiserror::Error;

/// An error that occurred during AXFR authorization.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum AxfrError {
    #[error("IP address is not allowed to request AXFR")]
    UnauthorizedIp,
    #[error("TSIG key not found")]
    KeyNotFound,
    #[error("Invalid TSIG signature")]
    InvalidSignature,
    #[error("TSIG timestamp is outside the allowed fudge window")]
    TimeFudgeExceeded,
    #[error("Invalid CIDR format")]
    InvalidCidr,
}

/// A parsed CIDR block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cidr {
    pub ip: IpAddr,
    pub prefix: u8,
}

impl FromStr for Cidr {
    type Err = AxfrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            // Assume /32 or /128
            let ip = IpAddr::from_str(s).map_err(|_| AxfrError::InvalidCidr)?;
            let prefix = if ip.is_ipv4() { 32 } else { 128 };
            return Ok(Cidr { ip, prefix });
        }
        let ip = IpAddr::from_str(parts[0]).map_err(|_| AxfrError::InvalidCidr)?;
        let prefix = u8::from_str(parts[1]).map_err(|_| AxfrError::InvalidCidr)?;

        let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
        if prefix > max_prefix {
            return Err(AxfrError::InvalidCidr);
        }

        Ok(Cidr { ip, prefix })
    }
}

impl Cidr {
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.ip, ip) {
            (IpAddr::V4(net_v4), IpAddr::V4(ip_v4)) => {
                let mask = if self.prefix == 0 { 0 } else { (!0u32) << (32 - self.prefix) };
                let net_u32 = u32::from(net_v4);
                let ip_u32 = u32::from(ip_v4);
                (ip_u32 & mask) == (net_u32 & mask)
            },
            (IpAddr::V6(net_v6), IpAddr::V6(ip_v6)) => {
                let mask = if self.prefix == 0 { 0 } else { (!0u128) << (128 - self.prefix) };
                let net_u128 = u128::from(net_v6);
                let ip_u128 = u128::from(ip_v6);
                (ip_u128 & mask) == (net_u128 & mask)
            },
            _ => false,
        }
    }
}

/// A TSIG configuration containing the key material.
#[derive(Debug, Clone)]
pub struct TsigKey {
    pub name: String,
    pub key: Vec<u8>,
}

/// Authorizer for handling incoming AXFR requests.
#[derive(Debug, Clone, Default)]
pub struct AxfrAuthorizer {
    allowed_cidrs: Vec<Cidr>,
    tsig_keys: HashMap<String, TsigKey>,
}

impl AxfrAuthorizer {
    pub fn new() -> Self {
        Self {
            allowed_cidrs: Vec::new(),
            tsig_keys: HashMap::new(),
        }
    }

    /// Adds a CIDR block that is allowed to request zone transfers.
    pub fn add_allowed_cidr(&mut self, cidr: &str) -> Result<(), AxfrError> {
        let parsed = Cidr::from_str(cidr)?;
        self.allowed_cidrs.push(parsed);
        Ok(())
    }

    /// Adds a TSIG key. The key should be base64-encoded.
    pub fn add_tsig_key(&mut self, name: &str, key_base64: &str) -> Result<(), AxfrError> {
        let key = decode_base64(key_base64).ok_or(AxfrError::InvalidSignature)?;
        self.tsig_keys.insert(name.to_string(), TsigKey { name: name.to_string(), key });
        Ok(())
    }

    /// Checks if a client IP is allowed.
    pub fn is_ip_allowed(&self, ip: IpAddr) -> bool {
        self.allowed_cidrs.iter().any(|c| c.contains(ip))
    }

    /// Validates a TSIG signature.
    /// * `key_name` - The name of the TSIG key provided in the request
    /// * `request_mac` - The MAC provided in the request
    /// * `message_data` - The wire format data of the DNS message that was signed
    /// * `request_time` - The timestamp provided in the TSIG record
    /// * `current_time` - The current time on the server
    /// * `fudge` - The allowed time skew (fudge) in seconds
    pub fn validate_tsig(
        &self,
        key_name: &str,
        request_mac: &[u8],
        message_data: &[u8],
        request_time: u64,
        current_time: u64,
        fudge: u16,
    ) -> Result<(), AxfrError> {
        // Check timestamp fudge
        let time_diff = if request_time > current_time {
            request_time - current_time
        } else {
            current_time - request_time
        };

        if time_diff > fudge as u64 {
            return Err(AxfrError::TimeFudgeExceeded);
        }

        // Get TSIG key
        let tsig_key = self.tsig_keys.get(key_name).ok_or(AxfrError::KeyNotFound)?;

        // Validate signature using HMAC-SHA256
        let computed_mac = hmac_sha256(&tsig_key.key, message_data);

        // Constant time comparison (simple for strings, but for MACs a constant time check is better)
        // Here we just use standard equality for simplicity unless strict constant time is required.
        // We'll use a simple byte-by-byte comparison that runs in constant time.
        if request_mac.len() != computed_mac.len() {
            return Err(AxfrError::InvalidSignature);
        }

        let mut diff = 0;
        for (a, b) in request_mac.iter().zip(computed_mac.iter()) {
            diff |= a ^ b;
        }

        if diff != 0 {
            return Err(AxfrError::InvalidSignature);
        }

        Ok(())
    }
}

// SHA-256 and HMAC-SHA256 Implementation (Safe Rust)

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

fn hmac_sha256(key: &[u8], data: &[u8]) -> [u8; 32] {
    let mut k = key.to_vec();
    if k.len() > 64 {
        k = sha256(&k).to_vec();
    }
    if k.len() < 64 {
        k.resize(64, 0);
    }

    let mut o_pad = [0x5cu8; 64];
    let mut i_pad = [0x36u8; 64];

    for i in 0..64 {
        o_pad[i] ^= k[i];
        i_pad[i] ^= k[i];
    }

    let mut inner_data = i_pad.to_vec();
    inner_data.extend_from_slice(data);
    let inner_hash = sha256(&inner_data);

    let mut outer_data = o_pad.to_vec();
    outer_data.extend_from_slice(&inner_hash);
    sha256(&outer_data)
}

fn decode_base64(input: &str) -> Option<Vec<u8>> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits = 0;

    for c in input.chars() {
        if c == '=' {
            break;
        }
        if c.is_whitespace() {
            continue;
        }
        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            _ => return None,
        };
        buffer = (buffer << 6) | val;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            output.push((buffer >> bits) as u8);
        }
    }
    Some(output)
}

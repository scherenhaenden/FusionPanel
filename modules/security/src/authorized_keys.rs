#![forbid(unsafe_code)]

use std::collections::HashSet;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
use std::path::Path;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum SshKeyError {
    #[error("Invalid key format")]
    InvalidFormat,
    #[error("Unsupported key type")]
    UnsupportedKeyType,
    #[error("Base64 decode error")]
    Base64DecodeError,
    #[error("IO error: {0}")]
    IoError(#[from] io::Error),
    #[error("File permissions error")]
    PermissionsError,
}

#[derive(Debug, Clone, Eq)]
pub struct SshKey {
    pub key_type: String,
    pub key_data: String,
    pub comment: Option<String>,
    pub fingerprint: String,
}

impl PartialEq for SshKey {
    fn eq(&self, other: &Self) -> bool {
        self.fingerprint == other.fingerprint
    }
}

impl std::hash::Hash for SshKey {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.fingerprint.hash(state);
    }
}

pub struct SshKeyManager {
    keys: HashSet<SshKey>,
}

impl SshKeyManager {
    pub fn new() -> Self {
        Self {
            keys: HashSet::new(),
        }
    }

    pub fn parse(content: &str) -> Result<Self, SshKeyError> {
        let mut manager = Self::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            if let Ok(key) = Self::parse_key(line) {
                manager.add_key(key);
            }
        }
        Ok(manager)
    }

    fn decode_base64(input: &str) -> Result<Vec<u8>, SshKeyError> {
        let mut output = Vec::new();
        let mut buffer = 0u32;
        let mut bits = 0;
        for c in input.chars() {
            if c == '=' { break; }
            let val = match c {
                'A'..='Z' => c as u32 - 'A' as u32,
                'a'..='z' => c as u32 - 'a' as u32 + 26,
                '0'..='9' => c as u32 - '0' as u32 + 52,
                '+' => 62,
                '/' => 63,
                _ => return Err(SshKeyError::Base64DecodeError),
            };
            buffer = (buffer << 6) | val;
            bits += 6;
            if bits >= 8 {
                bits -= 8;
                output.push((buffer >> bits) as u8);
            }
        }
        Ok(output)
    }

    fn encode_base64_no_pad(input: &[u8]) -> String {
        let charset = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut output = String::new();
        let mut buffer = 0u32;
        let mut bits = 0;
        for &b in input {
            buffer = (buffer << 8) | (b as u32);
            bits += 8;
            while bits >= 6 {
                bits -= 6;
                output.push(charset[((buffer >> bits) & 0x3F) as usize] as char);
            }
        }
        if bits > 0 {
            buffer <<= 6 - bits;
            output.push(charset[(buffer & 0x3F) as usize] as char);
        }
        output
    }

    fn sha256(input: &[u8]) -> [u8; 32] {
        let mut h: [u32; 8] = [
            0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
            0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
        ];

        let mut message = input.to_vec();
        let bit_len = (message.len() as u64) * 8;
        message.push(0x80);
        while (message.len() % 64) != 56 {
            message.push(0x00);
        }
        message.extend_from_slice(&bit_len.to_be_bytes());

        const K32: [u32; 64] = [
            0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5,
            0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
            0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3,
            0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
            0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc,
            0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
            0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7,
            0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
            0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13,
            0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
            0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3,
            0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
            0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5,
            0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
            0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208,
            0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
        ];

        for chunk in message.chunks(64) {
            let mut w = [0u32; 64];
            for i in 0..16 {
                let mut bytes = [0u8; 4];
                bytes.copy_from_slice(&chunk[i * 4..i * 4 + 4]);
                w[i] = u32::from_be_bytes(bytes);
            }
            for i in 16..64 {
                let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
                let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
                w[i] = w[i - 16].wrapping_add(s0).wrapping_add(w[i - 7]).wrapping_add(s1);
            }

            let mut a = h[0];
            let mut b = h[1];
            let mut c = h[2];
            let mut d = h[3];
            let mut e = h[4];
            let mut f = h[5];
            let mut g = h[6];
            let mut h_v = h[7];

            for i in 0..64 {
                let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
                let ch = (e & f) ^ (!e & g);
                let temp1 = h_v.wrapping_add(s1).wrapping_add(ch).wrapping_add(K32[i]).wrapping_add(w[i]);
                let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
                let maj = (a & b) ^ (a & c) ^ (b & c);
                let temp2 = s0.wrapping_add(maj);

                h_v = g;
                g = f;
                f = e;
                e = d.wrapping_add(temp1);
                d = c;
                c = b;
                b = a;
                a = temp1.wrapping_add(temp2);
            }

            h[0] = h[0].wrapping_add(a);
            h[1] = h[1].wrapping_add(b);
            h[2] = h[2].wrapping_add(c);
            h[3] = h[3].wrapping_add(d);
            h[4] = h[4].wrapping_add(e);
            h[5] = h[5].wrapping_add(f);
            h[6] = h[6].wrapping_add(g);
            h[7] = h[7].wrapping_add(h_v);
        }

        let mut result = [0u8; 32];
        for (i, &val) in h.iter().enumerate() {
            result[i * 4..i * 4 + 4].copy_from_slice(&val.to_be_bytes());
        }
        result
    }

    pub fn compute_fingerprint(key_data: &str) -> Result<String, SshKeyError> {
        let binary_key = Self::decode_base64(key_data)?;
        let hash = Self::sha256(&binary_key);
        Ok(format!("SHA256:{}", Self::encode_base64_no_pad(&hash)))
    }

    pub fn parse_key(line: &str) -> Result<SshKey, SshKeyError> {
        let mut parts = line.splitn(3, ' ');
        let key_type = parts.next().ok_or(SshKeyError::InvalidFormat)?;
        let key_data = parts.next().ok_or(SshKeyError::InvalidFormat)?;
        let comment = parts.next().map(|s| s.to_string());

        if !matches!(
            key_type,
            "ssh-ed25519"
                | "ssh-rsa"
                | "ecdsa-sha2-nistp256"
                | "ecdsa-sha2-nistp384"
                | "ecdsa-sha2-nistp521"
        ) {
            return Err(SshKeyError::UnsupportedKeyType);
        }

        let fingerprint = Self::compute_fingerprint(key_data)?;

        Ok(SshKey {
            key_type: key_type.to_string(),
            key_data: key_data.to_string(),
            comment,
            fingerprint,
        })
    }

    pub fn add_key(&mut self, key: SshKey) -> bool {
        self.keys.insert(key)
    }

    pub fn remove_key_by_fingerprint(&mut self, fingerprint: &str) -> bool {
        let initial_len = self.keys.len();
        self.keys.retain(|k| k.fingerprint != fingerprint);
        self.keys.len() < initial_len
    }
    
    pub fn get_keys(&self) -> Vec<&SshKey> {
        self.keys.iter().collect()
    }

    pub fn serialize(&self) -> String {
        let mut output = String::new();
        // Since HashSet does not preserve order, we might want to sort by fingerprint or just return as is.
        // Let's sort to have deterministic serialization.
        let mut keys: Vec<&SshKey> = self.keys.iter().collect();
        keys.sort_by(|a, b| a.fingerprint.cmp(&b.fingerprint));

        for key in keys {
            if let Some(comment) = &key.comment {
                output.push_str(&format!("{} {} {}\n", key.key_type, key.key_data, comment));
            } else {
                output.push_str(&format!("{} {}\n", key.key_type, key.key_data));
            }
        }
        output
    }

    pub fn write_to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), SshKeyError> {
        let content = self.serialize();
        
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;

        file.write_all(content.as_bytes())?;
        
        // Verify permissions
        let perms = fs::metadata(&path)?.permissions();
        if perms.mode() & 0o777 != 0o600 {
            return Err(SshKeyError::PermissionsError);
        }

        Ok(())
    }
}

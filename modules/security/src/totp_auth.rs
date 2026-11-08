#![forbid(unsafe_code)]

use std::collections::{HashSet, VecDeque};


#[derive(Debug, thiserror::Error)]
pub enum TotpError {
    #[error("Invalid Base32 secret")]
    InvalidSecret,
    #[error("Code expired or invalid")]
    InvalidCode,
    #[error("Code has already been used")]
    ReplayedCode,
}

const TIMESTEP: u64 = 30;

fn sha1(data: &[u8]) -> [u8; 20] {
    let mut h = [
        0x67452301u32,
        0xEFCDAB89u32,
        0x98BADCFEu32,
        0x10325476u32,
        0xC3D2E1F0u32,
    ];
    let ml = (data.len() * 8) as u64;
    let mut buf = Vec::with_capacity(data.len() + 64);
    buf.extend_from_slice(data);
    buf.push(0x80);
    while (buf.len() % 64) != 56 {
        buf.push(0x00);
    }
    buf.extend_from_slice(&ml.to_be_bytes());

    for chunk in buf.chunks_exact(64) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            w[i] = u32::from_be_bytes(chunk[i * 4..i * 4 + 4].try_into().unwrap());
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }

        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];

        for i in 0..80 {
            let (f, k) = match i {
                0..=19 => ((b & c) | (!b & d), 0x5A827999),
                20..=39 => (b ^ c ^ d, 0x6ED9EBA1),
                40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1BBCDC),
                60..=79 => (b ^ c ^ d, 0xCA62C1D6),
                _ => unreachable!(),
            };

            let temp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = temp;
        }

        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }

    let mut out = [0u8; 20];
    for i in 0..5 {
        let bytes = h[i].to_be_bytes();
        out[i * 4..i * 4 + 4].copy_from_slice(&bytes);
    }
    out
}

fn hmac_sha1(key: &[u8], message: &[u8]) -> [u8; 20] {
    let mut key_block = [0u8; 64];
    if key.len() > 64 {
        let hash = sha1(key);
        key_block[..20].copy_from_slice(&hash);
    } else {
        key_block[..key.len()].copy_from_slice(key);
    }

    let mut o_key_pad = [0x5cu8; 64];
    let mut i_key_pad = [0x36u8; 64];
    for i in 0..64 {
        o_key_pad[i] ^= key_block[i];
        i_key_pad[i] ^= key_block[i];
    }

    let mut inner_message = Vec::with_capacity(64 + message.len());
    inner_message.extend_from_slice(&i_key_pad);
    inner_message.extend_from_slice(message);
    let inner_hash = sha1(&inner_message);

    let mut outer_message = Vec::with_capacity(64 + 20);
    outer_message.extend_from_slice(&o_key_pad);
    outer_message.extend_from_slice(&inner_hash);
    sha1(&outer_message)
}

fn decode_base32(input: &str) -> Result<Vec<u8>, TotpError> {
    let mut output = Vec::new();
    let mut buffer: u32 = 0;
    let mut bits_left = 0;

    for c in input.chars() {
        if c == '=' {
            break;
        }
        if c.is_whitespace() {
            continue;
        }

        let val = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32,
            '2'..='7' => c as u32 - '2' as u32 + 26,
            _ => return Err(TotpError::InvalidSecret),
        };

        buffer = (buffer << 5) | val;
        bits_left += 5;

        if bits_left >= 8 {
            bits_left -= 8;
            output.push((buffer >> bits_left) as u8);
        }
    }

    Ok(output)
}

pub fn generate_totp(secret: &[u8], timestep: u64) -> String {
    let msg = timestep.to_be_bytes();
    let hash = hmac_sha1(secret, &msg);
    let offset = (hash[19] & 0x0f) as usize;
    let binary = ((hash[offset] as u32 & 0x7f) << 24)
        | ((hash[offset + 1] as u32 & 0xff) << 16)
        | ((hash[offset + 2] as u32 & 0xff) << 8)
        | (hash[offset + 3] as u32 & 0xff);
    let code = binary % 1_000_000;
    format!("{:06}", code)
}

pub struct TotpAuthenticator {
    used_codes: VecDeque<(String, u64)>, // user_secret, timestep
    used_set: HashSet<(String, u64)>,
}

impl TotpAuthenticator {
    pub fn new() -> Self {
        Self {
            used_codes: VecDeque::new(),
            used_set: HashSet::new(),
        }
    }

    fn prune_old_codes(&mut self, current_step: u64) {
        while let Some(&(_, step)) = self.used_codes.front() {
            if step + 1 < current_step {
                let entry = self.used_codes.pop_front().unwrap();
                self.used_set.remove(&entry);
            } else {
                break;
            }
        }
    }

    pub fn verify(&mut self, secret_b32: &str, code: &str, current_time: u64) -> Result<(), TotpError> {
        let secret = decode_base32(secret_b32)?;
        let current_step = current_time / TIMESTEP;

        self.prune_old_codes(current_step);

        for step in (current_step.saturating_sub(1))..=(current_step + 1) {
            let expected_code = generate_totp(&secret, step);
            if expected_code == code {
                let entry = (secret_b32.to_string(), step);
                if self.used_set.contains(&entry) {
                    return Err(TotpError::ReplayedCode);
                }
                self.used_codes.push_back(entry.clone());
                self.used_set.insert(entry);
                return Ok(());
            }
        }

        Err(TotpError::InvalidCode)
    }
}

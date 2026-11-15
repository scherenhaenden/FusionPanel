#![forbid(unsafe_code)]

use std::collections::{HashMap, VecDeque};


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    HmacSha256,
    HmacMd5,
}

#[derive(Debug, PartialEq, Eq)]
pub enum TsigError {
    KeyNotFound,
    InvalidSignature,
    TimeFudgeExceeded,
    ReplayDetected,
}

#[derive(Clone)]
struct Key {
    algorithm: Algorithm,
    secret: Vec<u8>,
}

pub struct TSIGKeyRing {
    keys: HashMap<String, Key>,
    // For replay prevention (tracking signature digests)
    recent_requests: VecDeque<(u64, Vec<u8>)>,
}

impl TSIGKeyRing {
    pub fn new() -> Self {
        Self {
            keys: HashMap::new(),
            recent_requests: VecDeque::new(),
        }
    }

    pub fn add_key(&mut self, name: &str, algorithm: Algorithm, secret: &[u8]) {
        self.keys.insert(
            name.to_string(),
            Key {
                algorithm,
                secret: secret.to_vec(),
            },
        );
    }

    pub fn verify_tsig_request(
        &mut self,
        key_name: &str,
        message_digest: &[u8], // The raw DNS message data
        expected_mac: &[u8],   // The MAC from the TSIG record
        request_time: u64,
        fudge: u64,
        current_time: u64,
    ) -> Result<(), TsigError> {
        let key = self.keys.get(key_name).ok_or(TsigError::KeyNotFound)?;

        // Check time bounds
        if current_time.abs_diff(request_time) > fudge {
            return Err(TsigError::TimeFudgeExceeded);
        }

        // Clean up old replay records
        let min_time = current_time.saturating_sub(fudge);
        while let Some(&(t, _)) = self.recent_requests.front() {
            if t < min_time {
                self.recent_requests.pop_front();
            } else {
                break;
            }
        }

        // Check for replay
        let mac_vec = expected_mac.to_vec();
        for &(t, ref mac) in &self.recent_requests {
            if t == request_time && mac == &mac_vec {
                return Err(TsigError::ReplayDetected);
            }
        }

        // Compute MAC
        let computed_mac = match key.algorithm {
            Algorithm::HmacSha256 => hmac_sha256(&key.secret, message_digest),
            Algorithm::HmacMd5 => hmac_md5(&key.secret, message_digest),
        };

        // Constant time comparison (simple for now since we just need basic strict safe rust, but we could make it CT)
        let mut diff = 0;
        if expected_mac.len() != computed_mac.len() {
            return Err(TsigError::InvalidSignature);
        }
        for (a, b) in expected_mac.iter().zip(computed_mac.iter()) {
            diff |= a ^ b;
        }

        if diff != 0 {
            return Err(TsigError::InvalidSignature);
        }

        // Record for replay prevention
        self.recent_requests.push_back((request_time, mac_vec));

        Ok(())
    }
}

// =========================================================================
// Crypto Implementations (Strict Safe Rust)
// =========================================================================

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

    let mut out = [0u8; 32];
    for i in 0..8 {
        out[i * 4..(i + 1) * 4].copy_from_slice(&hash[i].to_be_bytes());
    }
    out
}

fn md5(input: &[u8]) -> [u8; 16] {
    let mut state: [u32; 4] = [0x67452301, 0xefcdab89, 0x98badcfe, 0x10325476];

    let mut msg = input.to_vec();
    let bit_len = (msg.len() as u64) * 8;
    msg.push(0x80);
    while msg.len() % 64 != 56 {
        msg.push(0);
    }
    msg.extend_from_slice(&bit_len.to_le_bytes());

    let s: [u32; 64] = [
        7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22, 7, 12, 17, 22,
        5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20, 5, 9, 14, 20,
        4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23, 4, 11, 16, 23,
        6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21, 6, 10, 15, 21,
    ];

    let k: [u32; 64] = [
        0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee,
        0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
        0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be,
        0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
        0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa,
        0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
        0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed,
        0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
        0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c,
        0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
        0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05,
        0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
        0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039,
        0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
        0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1,
        0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
    ];

    for chunk in msg.chunks_exact(64) {
        let mut m = [0u32; 16];
        for i in 0..16 {
            m[i] = u32::from_le_bytes([chunk[i*4], chunk[i*4+1], chunk[i*4+2], chunk[i*4+3]]);
        }

        let mut a = state[0];
        let mut b = state[1];
        let mut c = state[2];
        let mut d = state[3];

        for i in 0..64 {
            let (f, g) = if i < 16 {
                ((b & c) | (!b & d), i)
            } else if i < 32 {
                ((d & b) | (!d & c), (5 * i + 1) % 16)
            } else if i < 48 {
                (b ^ c ^ d, (3 * i + 5) % 16)
            } else {
                (c ^ (b | !d), (7 * i) % 16)
            };

            let sum = a.wrapping_add(f).wrapping_add(k[i]).wrapping_add(m[g]);
            a = d;
            d = c;
            c = b;
            b = b.wrapping_add(sum.rotate_left(s[i]));
        }

        state[0] = state[0].wrapping_add(a);
        state[1] = state[1].wrapping_add(b);
        state[2] = state[2].wrapping_add(c);
        state[3] = state[3].wrapping_add(d);
    }

    let mut out = [0u8; 16];
    for i in 0..4 {
        out[i * 4..(i + 1) * 4].copy_from_slice(&state[i].to_le_bytes());
    }
    out
}

pub fn hmac_sha256(key: &[u8], data: &[u8]) -> Vec<u8> {
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
    sha256(&outer_data).to_vec()
}

pub fn hmac_md5(key: &[u8], data: &[u8]) -> Vec<u8> {
    let mut k = key.to_vec();
    if k.len() > 64 {
        k = md5(&k).to_vec();
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
    let inner_hash = md5(&inner_data);

    let mut outer_data = o_pad.to_vec();
    outer_data.extend_from_slice(&inner_hash);
    md5(&outer_data).to_vec()
}

#![forbid(unsafe_code)]

use std::convert::TryInto;
use std::fmt::Write;

/// Computes the MD5 hash of the given input and returns it as a lowercase hex string.
pub fn compute_md5(input: &[u8]) -> String {
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
            m[i] = u32::from_le_bytes(chunk[i * 4..(i + 1) * 4].try_into().unwrap());
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

    format!(
        "{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}{:02x}",
        state[0] as u8, (state[0] >> 8) as u8, (state[0] >> 16) as u8, (state[0] >> 24) as u8,
        state[1] as u8, (state[1] >> 8) as u8, (state[1] >> 16) as u8, (state[1] >> 24) as u8,
        state[2] as u8, (state[2] >> 8) as u8, (state[2] >> 16) as u8, (state[2] >> 24) as u8,
        state[3] as u8, (state[3] >> 8) as u8, (state[3] >> 16) as u8, (state[3] >> 24) as u8
    )
}

#[derive(Debug, Clone)]
pub struct FastcgiCacheZoneManager {
    pub base_path: String,
    pub zone_name: String,
    pub keys_zone_size: String,
    pub max_size: String,
    pub inactive: String,
}

impl FastcgiCacheZoneManager {
    pub fn new(
        base_path: &str,
        zone_name: &str,
        keys_zone_size: &str,
        max_size: &str,
        inactive: &str,
    ) -> Self {
        Self {
            base_path: base_path.to_string(),
            zone_name: zone_name.to_string(),
            keys_zone_size: keys_zone_size.to_string(),
            max_size: max_size.to_string(),
            inactive: inactive.to_string(),
        }
    }

    /// Constructs the raw cache key.
    pub fn generate_cache_key(&self, scheme: &str, method: &str, host: &str, request_uri: &str) -> String {
        format!("{}{}{}{}", scheme, method, host, request_uri)
    }

    /// Computes the fastcgi cache file path, mapping to levels=1:2
    pub fn compute_cache_file_path(&self, cache_key: &str) -> String {
        let hash = compute_md5(cache_key.as_bytes());
        let len = hash.len();
        // Nginx levels=1:2 means:
        // level 1: last character of md5
        // level 2: the two characters before the last character
        let l1 = &hash[len - 1..len];
        let l2 = &hash[len - 3..len - 1];

        let path = std::path::Path::new(&self.base_path)
            .join(l1)
            .join(l2)
            .join(&hash);

        path.to_string_lossy().into_owned()
    }

    /// Generates the `fastcgi_cache_path` config directive.
    pub fn generate_path_directive(&self) -> String {
        format!(
            "fastcgi_cache_path {} levels=1:2 keys_zone={}:{} max_size={} inactive={} use_temp_path=off;",
            self.base_path, self.zone_name, self.keys_zone_size, self.max_size, self.inactive
        )
    }

    /// Generates bypass and no_cache rules for cookies and args.
    pub fn generate_bypass_rules(&self) -> String {
        let rules = "$cookie_PHPSESSID $cookie_wp_logged_in_ $arg_nocache";
        let mut out = String::new();
        let _ = writeln!(&mut out, "fastcgi_cache_bypass {};", rules);
        let _ = write!(&mut out, "fastcgi_no_cache {};", rules);
        out
    }
}

#![forbid(unsafe_code)]

use sha2::{Sha256, Digest};
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use std::path::Path;

#[derive(Debug, PartialEq, Eq)]
pub enum VerificationResult {
    Valid,
    Mismatch { expected: String, actual: String },
    ReadError,
}

pub fn format_sha256_file(hash: &str, filename: &str) -> String {
    format!("{}  {}\n", hash, filename)
}

pub fn parse_sha256_file(contents: &str) -> Option<(String, String)> {
    let line = contents.lines().next()?;
    let (hash, rest) = line.split_once(' ')?;

    // Valid SHA-256 hash in hex is exactly 64 characters long
    if hash.len() != 64 || !hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }

    let filename = rest.trim_start_matches(|c| c == ' ' || c == '*');
    Some((hash.to_string(), filename.to_string()))
}

pub struct ChecksumVerifier;

impl ChecksumVerifier {
    pub async fn verify_file<P: AsRef<Path>>(path: P, expected_hash: &str) -> VerificationResult {
        let mut file = match File::open(path).await {
            Ok(f) => f,
            Err(_) => return VerificationResult::ReadError,
        };

        let mut hasher = Sha256::new();
        let mut buffer = [0u8; 8192];

        loop {
            match file.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => hasher.update(&buffer[..n]),
                Err(_) => return VerificationResult::ReadError,
            }
        }

        let result = hasher.finalize();
        let actual_hash = hex::encode(result);

        if actual_hash == expected_hash {
            VerificationResult::Valid
        } else {
            VerificationResult::Mismatch {
                expected: expected_hash.to_string(),
                actual: actual_hash,
            }
        }
    }
}

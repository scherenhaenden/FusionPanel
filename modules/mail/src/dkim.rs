#![forbid(unsafe_code)]

use std::fs::OpenOptions;
use std::io::Write;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::process::{Command, Stdio};

#[derive(Debug, PartialEq, Eq)]
pub enum DkimError {
    CommandError(String),
    IoError(String),
}

impl std::fmt::Display for DkimError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DkimError::CommandError(s) => write!(f, "Command error: {}", s),
            DkimError::IoError(s) => write!(f, "IO error: {}", s),
        }
    }
}

impl std::error::Error for DkimError {}

pub struct DkimKeyPair {
    pub private_key: String,
    pub public_key: String,
    #[allow(dead_code)]
    pub txt_record: String,
}

pub fn generate_dkim_keypair(private_key_path: &Path) -> Result<DkimKeyPair, DkimError> {
    // 1. Generate 2048-bit RSA private key using openssl
    let gen_output = Command::new("openssl")
        .args(["genrsa", "2048"])
        .output()
        .map_err(|e| DkimError::CommandError(format!("Failed to execute openssl genrsa: {}", e)))?;

    if !gen_output.status.success() {
        return Err(DkimError::CommandError(format!(
            "openssl genrsa failed: {}",
            String::from_utf8_lossy(&gen_output.stderr)
        )));
    }

    let private_key = String::from_utf8(gen_output.stdout)
        .map_err(|e| DkimError::CommandError(format!("Invalid UTF-8 in private key: {}", e)))?;

    // 2. Export private key with secure 0600 file permissions
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(private_key_path)
        .map_err(|e| DkimError::IoError(format!("Failed to open file: {}", e)))?;

    file.write_all(private_key.as_bytes())
        .map_err(|e| DkimError::IoError(format!("Failed to write private key: {}", e)))?;

    // 3. Generate public key using openssl rsa
    let mut rsa_child = Command::new("openssl")
        .args(["rsa", "-pubout"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| DkimError::CommandError(format!("Failed to spawn openssl rsa: {}", e)))?;

    if let Some(mut stdin) = rsa_child.stdin.take() {
        stdin
            .write_all(private_key.as_bytes())
            .map_err(|e| DkimError::CommandError(format!("Failed to write to stdin: {}", e)))?;
    }

    let rsa_output = rsa_child
        .wait_with_output()
        .map_err(|e| DkimError::CommandError(format!("Failed to wait for openssl rsa: {}", e)))?;

    if !rsa_output.status.success() {
        return Err(DkimError::CommandError(format!(
            "openssl rsa failed: {}",
            String::from_utf8_lossy(&rsa_output.stderr)
        )));
    }

    let public_key = String::from_utf8(rsa_output.stdout)
        .map_err(|e| DkimError::CommandError(format!("Invalid UTF-8 in public key: {}", e)))?;

    // 4. Format public key into standard DNS TXT record format
    let txt_record = format_dns_txt_record(&public_key)?;

    Ok(DkimKeyPair {
        private_key,
        public_key,
        txt_record,
    })
}

pub fn format_dns_txt_record(public_key: &str) -> Result<String, DkimError> {
    // Extract base64 part from PEM
    let mut base64_data = String::new();
    for line in public_key.lines() {
        let line = line.trim();
        if !line.starts_with("-----") && !line.is_empty() {
            base64_data.push_str(line);
        }
    }

    if base64_data.is_empty() {
        return Err(DkimError::CommandError(
            "Failed to extract base64 from public key".to_string(),
        ));
    }

    // Format into v=DKIM1; k=rsa; p=...
    let record = format!("v=DKIM1; k=rsa; p={}", base64_data);

    // DNS TXT chunking (<255 chars)
    let mut chunks = Vec::new();
    let mut current = record.as_str();

    while current.len() > 255 {
        chunks.push(format!("\"{}\"", &current[..255]));
        current = &current[255..];
    }

    if !current.is_empty() {
        chunks.push(format!("\"{}\"", current));
    }

    Ok(chunks.join(" "))
}

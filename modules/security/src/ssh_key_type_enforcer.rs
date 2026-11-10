#![forbid(unsafe_code)]


use thiserror::Error;

#[derive(Debug, Error)]
pub enum SshAuditorError {
    #[error("Invalid authorized_keys entry: {0}")]
    InvalidAuthorizedKeysEntry(String),
    #[error("Base64 decode error: {0}")]
    Base64DecodeError(String),
    #[error("Invalid key format: {0}")]
    InvalidKeyFormat(String),
    #[error("Invalid sshd_config line: {0}")]
    InvalidSshdConfigLine(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum KeyClassification {
    Allowed,
    Warning(String),
    Rejected(String),
}

#[derive(Debug, PartialEq, Eq)]
pub struct SshKeyAuditResult {
    pub key_type: String,
    pub classification: KeyClassification,
    pub comment: Option<String>,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SshdConfigAuditResult {
    pub weak_ciphers: Vec<String>,
    pub weak_macs: Vec<String>,
    pub weak_kex: Vec<String>,
    pub recommendations: Vec<String>,
}

pub struct SshKeySecurityAuditor {
    pub min_rsa_size_allowed: usize,
    pub min_rsa_size_warning: usize,
}

impl Default for SshKeySecurityAuditor {
    fn default() -> Self {
        Self {
            min_rsa_size_allowed: 2048,
            min_rsa_size_warning: 3072,
        }
    }
}

fn decode_base64(input: &str) -> Result<Vec<u8>, SshAuditorError> {
    let mut output = Vec::new();
    let mut buffer = 0u32;
    let mut bits_in_buffer = 0;

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
            _ => return Err(SshAuditorError::Base64DecodeError(format!("Invalid character: {}", c))),
        };

        buffer = (buffer << 6) | val;
        bits_in_buffer += 6;

        if bits_in_buffer >= 8 {
            bits_in_buffer -= 8;
            output.push((buffer >> bits_in_buffer) as u8);
        }
    }

    Ok(output)
}

fn read_u32(data: &[u8], offset: &mut usize) -> Result<u32, SshAuditorError> {
    if *offset + 4 > data.len() {
        return Err(SshAuditorError::InvalidKeyFormat("Unexpected end of data".to_string()));
    }
    let val = u32::from_be_bytes(data[*offset..*offset + 4].try_into().unwrap());
    *offset += 4;
    Ok(val)
}

fn read_string<'a>(data: &'a [u8], offset: &mut usize) -> Result<&'a [u8], SshAuditorError> {
    let len = read_u32(data, offset)? as usize;
    if *offset + len > data.len() {
        return Err(SshAuditorError::InvalidKeyFormat("Unexpected end of data".to_string()));
    }
    let slice = &data[*offset..*offset + len];
    *offset += len;
    Ok(slice)
}

fn get_rsa_key_size(data: &[u8]) -> Result<usize, SshAuditorError> {
    let mut offset = 0;
    let key_type = read_string(data, &mut offset)?;
    if key_type != b"ssh-rsa" {
        return Err(SshAuditorError::InvalidKeyFormat("Not an RSA key".to_string()));
    }
    let _e = read_string(data, &mut offset)?;
    let n = read_string(data, &mut offset)?;

    let mut bit_len = n.len() * 8;
    if !n.is_empty() {
        let mut i = 0;
        while i < n.len() && n[i] == 0 {
            bit_len -= 8;
            i += 1;
        }
        if i < n.len() {
            let first_byte = n[i];
            let leading_zeros = first_byte.leading_zeros() as usize;
            bit_len -= leading_zeros;
        }
    }

    Ok(bit_len)
}

impl SshKeySecurityAuditor {
    pub fn new(min_rsa_size_allowed: usize, min_rsa_size_warning: usize) -> Self {
        Self {
            min_rsa_size_allowed,
            min_rsa_size_warning,
        }
    }

    pub fn audit_authorized_keys(&self, content: &str) -> Result<Vec<SshKeyAuditResult>, SshAuditorError> {
        let mut results = Vec::new();
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.is_empty() {
                continue;
            }

            let mut key_type_idx = None;
            for (i, part) in parts.iter().enumerate() {
                if part.starts_with("ssh-") || part.starts_with("ecdsa-") {
                    key_type_idx = Some(i);
                    break;
                }
            }

            if let Some(idx) = key_type_idx {
                if idx + 1 >= parts.len() {
                    return Err(SshAuditorError::InvalidAuthorizedKeysEntry(line.to_string()));
                }
                let key_type = parts[idx];
                let base64_data = parts[idx + 1];
                let comment = if parts.len() > idx + 2 {
                    Some(parts[idx + 2..].join(" "))
                } else {
                    None
                };

                let decoded = decode_base64(base64_data)?;

                let classification = match key_type {
                    "ssh-dss" => KeyClassification::Rejected("ssh-dss (DSA) keys are deprecated and insecure".to_string()),
                    "ssh-rsa" => {
                        let size = get_rsa_key_size(&decoded)?;
                        if size < self.min_rsa_size_allowed {
                            KeyClassification::Rejected(format!("RSA key size {} is below allowed minimum {}", size, self.min_rsa_size_allowed))
                        } else if size < self.min_rsa_size_warning {
                            KeyClassification::Warning(format!("RSA key size {} is below recommended minimum {}", size, self.min_rsa_size_warning))
                        } else {
                            KeyClassification::Allowed
                        }
                    },
                    "ssh-ed25519" => KeyClassification::Allowed,
                    "ecdsa-sha2-nistp256" | "ecdsa-sha2-nistp384" | "ecdsa-sha2-nistp521" => KeyClassification::Allowed,
                    _ => KeyClassification::Warning(format!("Unknown or unverified key type: {}", key_type)),
                };

                results.push(SshKeyAuditResult {
                    key_type: key_type.to_string(),
                    classification,
                    comment,
                });
            } else {
                return Err(SshAuditorError::InvalidAuthorizedKeysEntry(line.to_string()));
            }
        }
        Ok(results)
    }

    pub fn audit_sshd_config(&self, content: &str) -> SshdConfigAuditResult {
        let mut weak_ciphers = Vec::new();
        let mut weak_macs = Vec::new();
        let mut weak_kex = Vec::new();
        let mut recommendations = Vec::new();

        let weak_ciphers_list = ["3des-cbc", "arcfour", "arcfour128", "arcfour256", "blowfish-cbc", "cast128-cbc", "des-cbc"];
        let weak_macs_list = ["hmac-md5", "hmac-md5-96", "hmac-sha1", "hmac-sha1-96"];
        let weak_kex_list = ["diffie-hellman-group1-sha1", "diffie-hellman-group14-sha1"];

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let key = parts[0].to_lowercase();
                let values_str = parts[1..].join("");
                let items: Vec<&str> = values_str.split(',').collect();

                match key.as_str() {
                    "ciphers" => {
                        for item in items {
                            let clean_item = item.trim_start_matches(['+', '-', '^', '!']);
                            if weak_ciphers_list.contains(&clean_item) {
                                weak_ciphers.push(clean_item.to_string());
                            }
                        }
                    }
                    "macs" => {
                        for item in items {
                            let clean_item = item.trim_start_matches(['+', '-', '^', '!']);
                            if weak_macs_list.contains(&clean_item) {
                                weak_macs.push(clean_item.to_string());
                            }
                        }
                    }
                    "kexalgorithms" => {
                        for item in items {
                            let clean_item = item.trim_start_matches(['+', '-', '^', '!']);
                            if weak_kex_list.contains(&clean_item) {
                                weak_kex.push(clean_item.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if !weak_ciphers.is_empty() {
            recommendations.push(format!("Remove weak ciphers: {}", weak_ciphers.join(", ")));
        }
        if !weak_macs.is_empty() {
            recommendations.push(format!("Remove weak MACs: {}", weak_macs.join(", ")));
        }
        if !weak_kex.is_empty() {
            recommendations.push(format!("Remove weak KEX algorithms: {}", weak_kex.join(", ")));
        }

        SshdConfigAuditResult {
            weak_ciphers,
            weak_macs,
            weak_kex,
            recommendations,
        }
    }
}

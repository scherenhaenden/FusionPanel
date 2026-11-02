use serde::{Deserialize, Serialize};
use std::fmt;
use std::process::Command;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DnsRecordType {
    MX,
    TXT,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub struct DnsRecord {
    pub name: String,
    pub record_type: DnsRecordType,
    pub value: String,
    pub priority: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
pub enum DkimKeyType {
    Rsa,
    Ed25519,
}

impl fmt::Display for DkimKeyType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DkimKeyType::Rsa => write!(f, "rsa"),
            DkimKeyType::Ed25519 => write!(f, "ed25519"),
        }
    }
}

pub struct DnsGenerator {
    pub domain: String,
}

impl DnsGenerator {
    pub fn new(domain: &str) -> Result<Self, String> {
        // Validate domain format simply for this module (ASCII).
        if !domain.is_ascii() || domain.is_empty() {
            return Err("Invalid domain".to_string());
        }
        Ok(Self {
            domain: domain.to_string(),
        })
    }

    pub fn generate_mx(&self, mail_server: &str) -> DnsRecord {
        DnsRecord {
            name: self.domain.clone(),
            record_type: DnsRecordType::MX,
            value: mail_server.to_string(),
            priority: Some(10),
        }
    }

    pub fn generate_spf(&self, allowed_ips: &[&str], include_domains: &[&str]) -> DnsRecord {
        let mut parts = vec!["v=spf1".to_string()];

        for ip in allowed_ips {
            // Very simple escaping, realistically IPv4/v6 addresses shouldn't contain spaces/quotes,
            // but for completeness we can just assert they are valid or safely formatted
            if ip.contains(':') {
                parts.push(format!("ip6:{}", ip.replace('"', "")));
            } else {
                parts.push(format!("ip4:{}", ip.replace('"', "")));
            }
        }

        for inc in include_domains {
            parts.push(format!("include:{}", inc.replace('"', "")));
        }

        parts.push("-all".to_string());

        let value = format!("\"{}\"", parts.join(" "));

        DnsRecord {
            name: self.domain.clone(),
            record_type: DnsRecordType::TXT,
            value,
            priority: None,
        }
    }

    pub fn generate_dmarc(&self, rua_email: &str) -> DnsRecord {
        // Escape rua_email quotes to prevent TXT record injection
        let escaped_rua = rua_email.replace('"', "");
        let value = format!("\"v=DMARC1; p=reject; rua=mailto:{};\"", escaped_rua);
        DnsRecord {
            name: format!("_dmarc.{}", self.domain),
            record_type: DnsRecordType::TXT,
            value,
            priority: None,
        }
    }
}

pub struct DkimKeyPair {
    pub selector: String,
    pub private_key: String,
    pub txt_record: DnsRecord,
}

impl DnsGenerator {
    pub fn generate_dkim(
        &self,
        selector: &str,
        key_type: DkimKeyType,
    ) -> Result<DkimKeyPair, String> {
        // Validate selector format to prevent path traversal
        if !selector
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-')
            || selector.is_empty()
        {
            return Err("Invalid selector format".to_string());
        }

        let temp_dir =
            tempfile::tempdir().map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let temp_dir_path = temp_dir.path();

        let key_path = temp_dir_path.join(format!("{}.private", selector));
        let txt_path = temp_dir_path.join(format!("{}.txt", selector));

        let mut cmd = Command::new("opendkim-genkey");
        cmd.arg("-s")
            .arg(selector)
            .arg("-d")
            .arg(&self.domain)
            .arg("-D")
            .arg(temp_dir_path);

        match key_type {
            DkimKeyType::Rsa => {
                cmd.arg("-b").arg("2048"); // default reasonable RSA size
            }
            DkimKeyType::Ed25519 => {
                cmd.arg("-a").arg("ed25519");
            }
        }

        let output = cmd
            .output()
            .map_err(|e| format!("BLOCKED: failed to execute opendkim-genkey: {}", e))?;

        if !output.status.success() {
            return Err(format!(
                "BLOCKED: opendkim-genkey failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let private_key = std::fs::read_to_string(&key_path)
            .map_err(|e| format!("Failed to read private key: {}", e))?;

        let txt_content = std::fs::read_to_string(&txt_path)
            .map_err(|e| format!("Failed to read txt record: {}", e))?;

        // Parse txt_content to extract the value part
        // Example: selector._domainkey IN TXT ( "v=DKIM1; k=rsa; " "p=..." )
        // OpemDKIM wraps long lines by splitting them into multiple quoted strings
        // surrounded by parentheses.
        let mut parsed_value = String::new();
        let mut in_quotes = false;

        for line in txt_content.lines() {
            let line = line.trim();
            if line.starts_with(';') {
                continue;
            }
            for c in line.chars() {
                if c == '"' {
                    in_quotes = !in_quotes;
                } else if in_quotes {
                    parsed_value.push(c);
                }
            }
        }

        // The strings should simply be concatenated without stripping all whitespace
        // to maintain notes and valid spaces.
        // We just reconstruct it into a single quoted string for our output format.
        let value = format!("\"{}\"", parsed_value);

        Ok(DkimKeyPair {
            selector: selector.to_string(),
            private_key,
            txt_record: DnsRecord {
                name: format!("{}._domainkey.{}", selector, self.domain),
                record_type: DnsRecordType::TXT,
                value,
                priority: None,
            },
        })
    }
}

#![forbid(unsafe_code)]

use std::net::IpAddr;
use std::str::FromStr;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ConfigError {
    #[error("Invalid IP address")]
    InvalidIp,
    #[error("Invalid CIDR format")]
    InvalidCidr,
    #[error("Input contains forbidden characters")]
    SyntaxInjection,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForwardPolicy {
    Only,
    First,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindForwarderConfig {
    pub policy: ForwardPolicy,
    pub forwarders: Vec<IpAddr>,
    pub allow_recursion: Vec<String>,
}

impl Default for BindForwarderConfig {
    fn default() -> Self {
        Self {
            policy: ForwardPolicy::First,
            forwarders: Vec::new(),
            allow_recursion: Vec::new(),
        }
    }
}

impl BindForwarderConfig {
    pub fn new(policy: ForwardPolicy) -> Self {
        Self {
            policy,
            forwarders: Vec::new(),
            allow_recursion: Vec::new(),
        }
    }

    fn check_syntax_injection(s: &str) -> Result<(), ConfigError> {
        if s.contains('{') || s.contains('}') || s.contains(';') || s.contains('"') || s.contains('\'') || s.contains('\n') || s.contains('\r') {
            return Err(ConfigError::SyntaxInjection);
        }
        Ok(())
    }

    pub fn add_forwarder(&mut self, ip_str: &str) -> Result<(), ConfigError> {
        Self::check_syntax_injection(ip_str)?;
        let ip = IpAddr::from_str(ip_str).map_err(|_| ConfigError::InvalidIp)?;
        self.forwarders.push(ip);
        Ok(())
    }

    pub fn add_recursion_acl(&mut self, cidr_str: &str) -> Result<(), ConfigError> {
        Self::check_syntax_injection(cidr_str)?;

        let parts: Vec<&str> = cidr_str.split('/').collect();
        if parts.len() > 2 {
            return Err(ConfigError::InvalidCidr);
        }

        let ip = IpAddr::from_str(parts[0]).map_err(|_| ConfigError::InvalidCidr)?;

        if parts.len() == 2 {
            let prefix = u8::from_str(parts[1]).map_err(|_| ConfigError::InvalidCidr)?;
            let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
            if prefix > max_prefix {
                return Err(ConfigError::InvalidCidr);
            }
        }

        self.allow_recursion.push(cidr_str.to_string());
        Ok(())
    }

    pub fn generate_config(&self) -> String {
        let mut config = String::new();

        if !self.forwarders.is_empty() {
            config.push_str("forwarders { ");
            for ip in &self.forwarders {
                config.push_str(&format!("{}; ", ip));
            }
            config.push_str("};\n");

            match self.policy {
                ForwardPolicy::Only => config.push_str("forward only;\n"),
                ForwardPolicy::First => config.push_str("forward first;\n"),
            }
        }

        if !self.allow_recursion.is_empty() {
            config.push_str("allow-recursion { ");
            for acl in &self.allow_recursion {
                config.push_str(&format!("{}; ", acl));
            }
            config.push_str("};\n");
        }

        config
    }
}

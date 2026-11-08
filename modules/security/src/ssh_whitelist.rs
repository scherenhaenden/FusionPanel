#![forbid(unsafe_code)]

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CidrError {
    InvalidFormat,
    InvalidIp,
    InvalidPrefix,
}

impl std::fmt::Display for CidrError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CidrError::InvalidFormat => write!(f, "Invalid CIDR format"),
            CidrError::InvalidIp => write!(f, "Invalid IP address in CIDR"),
            CidrError::InvalidPrefix => write!(f, "Invalid prefix length in CIDR"),
        }
    }
}

impl std::error::Error for CidrError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpCidr {
    V4(Ipv4Addr, u8),
    V6(Ipv6Addr, u8),
}

impl IpCidr {
    pub fn parse(s: &str) -> Result<Self, CidrError> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() != 2 {
            return Err(CidrError::InvalidFormat);
        }
        let ip = IpAddr::from_str(parts[0]).map_err(|_| CidrError::InvalidIp)?;
        let prefix = u8::from_str(parts[1]).map_err(|_| CidrError::InvalidPrefix)?;

        match ip {
            IpAddr::V4(ipv4) => {
                if prefix > 32 {
                    return Err(CidrError::InvalidPrefix);
                }
                Ok(IpCidr::V4(ipv4, prefix))
            }
            IpAddr::V6(ipv6) => {
                if prefix > 128 {
                    return Err(CidrError::InvalidPrefix);
                }
                Ok(IpCidr::V6(ipv6, prefix))
            }
        }
    }

    pub fn contains(&self, ip: &IpAddr) -> bool {
        match (self, ip) {
            (IpCidr::V4(network, prefix), IpAddr::V4(target)) => {
                if *prefix == 0 {
                    return true;
                }
                let network_val = u32::from(*network);
                let target_val = u32::from(*target);
                let mask = !0u32 << (32 - *prefix);
                (network_val & mask) == (target_val & mask)
            }
            (IpCidr::V6(network, prefix), IpAddr::V6(target)) => {
                if *prefix == 0 {
                    return true;
                }
                let network_val = u128::from(*network);
                let target_val = u128::from(*target);
                let mask = !0u128 << (128 - *prefix);
                (network_val & mask) == (target_val & mask)
            }
            _ => false,
        }
    }
}

impl std::fmt::Display for IpCidr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IpCidr::V4(ip, prefix) => write!(f, "{}/{}", ip, prefix),
            IpCidr::V6(ip, prefix) => write!(f, "{}/{}", ip, prefix),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    InvalidPort(u16),
    InvalidCidr(CidrError),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EngineError::InvalidPort(p) => write!(f, "Invalid port: {}", p),
            EngineError::InvalidCidr(e) => write!(f, "Invalid CIDR: {}", e),
        }
    }
}

impl std::error::Error for EngineError {}

#[derive(Debug, Clone)]
pub struct SshWhitelistEngine {
    ports: Vec<u16>,
    cidrs: Vec<IpCidr>,
}

impl SshWhitelistEngine {
    pub fn new() -> Self {
        Self {
            ports: Vec::new(),
            cidrs: Vec::new(),
        }
    }

    pub fn add_port(&mut self, port: u16) -> Result<(), EngineError> {
        if port == 0 {
            return Err(EngineError::InvalidPort(port));
        }
        if !self.ports.contains(&port) {
            self.ports.push(port);
        }
        Ok(())
    }

    pub fn add_cidr(&mut self, cidr_str: &str) -> Result<(), EngineError> {
        let cidr = IpCidr::parse(cidr_str).map_err(EngineError::InvalidCidr)?;
        if !self.cidrs.contains(&cidr) {
            self.cidrs.push(cidr);
        }
        Ok(())
    }

    pub fn is_ip_allowed(&self, ip: &IpAddr) -> bool {
        if self.cidrs.is_empty() {
            // If no whitelist is defined, we assume all IPs are blocked or allowed?
            // "Checks whether a given client IP address is allowed to connect to SSH under the whitelist policy."
            // Typically if whitelist is present, only those are allowed.
            // If empty, none are allowed. We'll return false if empty.
            return false;
        }
        for cidr in &self.cidrs {
            if cidr.contains(ip) {
                return true;
            }
        }
        false
    }

    pub fn generate_sshd_config(&self) -> String {
        let mut config = String::new();

        for port in &self.ports {
            config.push_str(&format!("Port {}\n", port));
        }

        if !self.cidrs.is_empty() {
            let addrs: Vec<String> = self.cidrs.iter().map(|c| c.to_string()).collect();
            config.push_str(&format!("Match Address {}\n", addrs.join(",")));
            // Usually we add some rules inside Match, but the prompt just says:
            // "Generates compliant OpenSSH sshd_config blocks (e.g. 'Port <p>', 'Match Address <cidr>')."
        }

        config
    }
}

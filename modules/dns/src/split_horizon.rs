#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cidr {
    V4(Ipv4Addr, u8),
    V6(Ipv6Addr, u8),
}

impl Cidr {
    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self, ip) {
            (Cidr::V4(net_ip, prefix), IpAddr::V4(client_ip)) => {
                let mask = if *prefix == 0 {
                    0
                } else {
                    (!0u32).checked_shl(32 - (*prefix as u32)).unwrap_or(0)
                };
                (u32::from(*net_ip) & mask) == (u32::from(client_ip) & mask)
            }
            (Cidr::V6(net_ip, prefix), IpAddr::V6(client_ip)) => {
                let mask = if *prefix == 0 {
                    0
                } else {
                    (!0u128).checked_shl(128 - (*prefix as u32)).unwrap_or(0)
                };
                (u128::from(*net_ip) & mask) == (u128::from(client_ip) & mask)
            }
            _ => false,
        }
    }
}

impl FromStr for Cidr {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('/').collect();
        if parts.len() > 2 {
            return Err("Invalid CIDR format".to_string());
        }

        let ip_str = parts[0];
        let ip = IpAddr::from_str(ip_str).map_err(|e| e.to_string())?;

        match ip {
            IpAddr::V4(v4) => {
                let prefix = if parts.len() == 2 {
                    parts[1].parse::<u8>().map_err(|e| e.to_string())?
                } else {
                    32
                };
                if prefix > 32 {
                    return Err("Invalid IPv4 prefix".to_string());
                }
                Ok(Cidr::V4(v4, prefix))
            }
            IpAddr::V6(v6) => {
                let prefix = if parts.len() == 2 {
                    parts[1].parse::<u8>().map_err(|e| e.to_string())?
                } else {
                    128
                };
                if prefix > 128 {
                    return Err("Invalid IPv6 prefix".to_string());
                }
                Ok(Cidr::V6(v6, prefix))
            }
        }
    }
}

pub fn is_internal_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_private() || 
            v4.is_loopback() || 
            // CGNAT 100.64.0.0/10
            (u32::from(v4) & 0xFFC0_0000) == 0x6440_0000
        }
        IpAddr::V6(v6) => {
            // Unique Local Address (ULA) fc00::/7
            (v6.segments()[0] & 0xfe00) == 0xfc00 ||
            v6.is_loopback()
        }
    }
}

#[derive(Debug, Clone)]
pub struct DnsView {
    pub name: String,
    pub is_internal: bool,
    pub acls: Vec<Cidr>,
    pub records: HashMap<String, Vec<IpAddr>>,
}

impl DnsView {
    pub fn new(name: &str, is_internal: bool) -> Self {
        Self {
            name: name.to_string(),
            is_internal,
            acls: Vec::new(),
            records: HashMap::new(),
        }
    }

    pub fn add_acl(&mut self, cidr: Cidr) {
        self.acls.push(cidr);
    }

    pub fn add_record(&mut self, domain: &str, ip: IpAddr) {
        self.records.entry(domain.to_string()).or_default().push(ip);
    }
}

pub struct SplitHorizonResolver {
    pub views: Vec<DnsView>,
    pub default_view: Option<DnsView>,
}

impl Default for SplitHorizonResolver {
    fn default() -> Self {
        Self::new()
    }
}

impl SplitHorizonResolver {
    pub fn new() -> Self {
        Self {
            views: Vec::new(),
            default_view: None,
        }
    }

    pub fn add_view(&mut self, view: DnsView) {
        self.views.push(view);
    }

    pub fn set_default_view(&mut self, view: DnsView) {
        self.default_view = Some(view);
    }

    pub fn resolve(&self, client_ip: IpAddr, domain: &str) -> Option<Vec<IpAddr>> {
        let matched_view = self.views.iter().find(|view| {
            view.acls.iter().any(|acl| acl.contains(client_ip))
        }).or(self.default_view.as_ref());

        let view = matched_view?;

        let mut ips = view.records.get(domain).cloned().unwrap_or_default();

        if !view.is_internal {
            ips.retain(|&ip| !is_internal_ip(ip));
        }

        if ips.is_empty() {
            None
        } else {
            Some(ips)
        }
    }
}

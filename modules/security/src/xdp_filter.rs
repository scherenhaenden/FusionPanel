#![forbid(unsafe_code)]

use std::fmt;
use std::net::IpAddr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XdpAction {
    Pass,
    Drop,
}

impl fmt::Display for XdpAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            XdpAction::Pass => write!(f, "XDP_PASS"),
            XdpAction::Drop => write!(f, "XDP_DROP"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CidrBlock {
    pub ip: IpAddr,
    pub prefix: u8,
}

impl CidrBlock {
    pub fn new(ip: IpAddr, prefix: u8) -> Result<Self, &'static str> {
        let max_prefix = if ip.is_ipv4() { 32 } else { 128 };
        if prefix > max_prefix {
            return Err("Invalid prefix length");
        }
        Ok(Self { ip, prefix })
    }
}

#[derive(Debug, Clone)]
pub struct XdpFilterPolicy {
    pub interface: String,
    pub allowlist: Vec<CidrBlock>,
    pub blacklist: Vec<IpAddr>,
    pub syn_flood_mitigation: bool,
    pub default_action: XdpAction,
}

impl XdpFilterPolicy {
    pub fn new(interface: impl Into<String>) -> Self {
        Self {
            interface: interface.into(),
            allowlist: Vec::new(),
            blacklist: Vec::new(),
            syn_flood_mitigation: false,
            default_action: XdpAction::Pass,
        }
    }

    pub fn with_syn_flood_mitigation(mut self, enabled: bool) -> Self {
        self.syn_flood_mitigation = enabled;
        self
    }

    pub fn with_default_action(mut self, action: XdpAction) -> Self {
        self.default_action = action;
        self
    }

    pub fn allow_cidr(mut self, cidr: CidrBlock) -> Self {
        self.allowlist.push(cidr);
        self
    }

    pub fn block_ip(mut self, ip: IpAddr) -> Self {
        self.blacklist.push(ip);
        self
    }

    pub fn format_bpf_maps(&self) -> String {
        let mut out = String::new();
        out.push_str(&format!("# XDP BPF Map Configuration for {}\n", self.interface));
        out.push_str(&format!("DEFAULT_ACTION={}\n", self.default_action));
        out.push_str(&format!(
            "SYN_FLOOD_MITIGATION={}\n",
            if self.syn_flood_mitigation { "ENABLED" } else { "DISABLED" }
        ));

        out.push_str("\n# ALLOWLIST_MAP (LPM Trie)\n");
        if self.allowlist.is_empty() {
            out.push_str("<empty>\n");
        } else {
            for cidr in &self.allowlist {
                out.push_str(&format!("ALLOW {}/{}\n", cidr.ip, cidr.prefix));
            }
        }

        out.push_str("\n# BLACKLIST_MAP (Hash Map)\n");
        if self.blacklist.is_empty() {
            out.push_str("<empty>\n");
        } else {
            for ip in &self.blacklist {
                out.push_str(&format!("BLOCK {}\n", ip));
            }
        }

        out
    }
}

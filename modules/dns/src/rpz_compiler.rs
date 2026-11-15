#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RpzAction {
    NxDomain,
    NoData,
    Cname(String),
    A(String),
    Aaaa(String),
}

impl RpzAction {
    pub fn to_rule_string(&self) -> String {
        match self {
            RpzAction::NxDomain => ".".to_string(),
            RpzAction::NoData => "*.".to_string(),
            RpzAction::Cname(target) => {
                if target.ends_with('.') {
                    target.clone()
                } else {
                    format!("{}.", target)
                }
            }
            RpzAction::A(ip) => ip.clone(),
            RpzAction::Aaaa(ip) => ip.clone(),
        }
    }
}

pub struct RpzCompiler {
    zone_name: String,
    ttl: u32,
    serial: u32,
    refresh: u32,
    retry: u32,
    expire: u32,
    minimum: u32,
    rules: BTreeMap<String, RpzAction>,
    include_wildcards: bool,
}

impl RpzCompiler {
    pub fn new(zone_name: impl Into<String>) -> Self {
        Self {
            zone_name: zone_name.into(),
            ttl: 86400,
            serial: 1,
            refresh: 3600,
            retry: 1800,
            expire: 604800,
            minimum: 86400,
            rules: BTreeMap::new(),
            include_wildcards: true,
        }
    }

    pub fn with_ttl(mut self, ttl: u32) -> Self {
        self.ttl = ttl;
        self
    }

    pub fn with_serial(mut self, serial: u32) -> Self {
        self.serial = serial;
        self
    }

    pub fn include_wildcards(mut self, include: bool) -> Self {
        self.include_wildcards = include;
        self
    }

    pub fn add_rule(&mut self, domain: impl Into<String>, action: RpzAction) {
        let mut domain = domain.into();
        domain = domain.trim().to_string();
        if domain.ends_with('.') {
            domain.pop();
        }
        self.rules.insert(domain, action);
    }

    pub fn parse_feed(&mut self, feed: &str, action: RpzAction) {
        for line in feed.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') || line.starts_with(';') {
                continue;
            }

            // Extract the domain part (handle /etc/hosts format or just plain domain list)
            let domain = if line.find(|c: char| c.is_whitespace()).is_some() {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() >= 2 && (parts[0] == "127.0.0.1" || parts[0] == "0.0.0.0") {
                    parts[1]
                } else {
                    parts[0]
                }
            } else {
                line
            };

            // Remove any trailing comments on the same line if present
            let domain = if let Some(idx) = domain.find('#') {
                &domain[..idx]
            } else {
                domain
            };

            let mut domain = domain.trim().to_string();

            // basic normalization
            domain = domain.to_lowercase();
            if domain.ends_with('.') {
                domain.pop();
            }

            if !domain.is_empty() {
                self.add_rule(domain, action.clone());
            }
        }
    }

    pub fn compile(&self) -> String {
        let mut out = String::new();
        let _zone_fqdn = if self.zone_name.ends_with('.') {
            self.zone_name.clone()
        } else {
            format!("{}.", self.zone_name)
        };

        writeln!(&mut out, "$TTL {}", self.ttl).unwrap();
        writeln!(&mut out, "@ IN SOA localhost. root.localhost. (").unwrap();
        writeln!(&mut out, "    {} ; Serial", self.serial).unwrap();
        writeln!(&mut out, "    {} ; Refresh", self.refresh).unwrap();
        writeln!(&mut out, "    {} ; Retry", self.retry).unwrap();
        writeln!(&mut out, "    {} ; Expire", self.expire).unwrap();
        writeln!(&mut out, "    {} ) ; Minimum TTL", self.minimum).unwrap();
        writeln!(&mut out, "@ IN NS localhost.").unwrap();
        writeln!(&mut out).unwrap();

        for (domain, action) in &self.rules {
            let rule_str = action.to_rule_string();

            // Domain record
            let record = match action {
                RpzAction::A(_) => format!("{} IN A {}", domain, rule_str),
                RpzAction::Aaaa(_) => format!("{} IN AAAA {}", domain, rule_str),
                _ => format!("{} IN CNAME {}", domain, rule_str),
            };
            writeln!(&mut out, "{}", record).unwrap();

            // Wildcard record
            if self.include_wildcards {
                let wc_record = match action {
                    RpzAction::A(_) => format!("*.{} IN A {}", domain, rule_str),
                    RpzAction::Aaaa(_) => format!("*.{} IN AAAA {}", domain, rule_str),
                    _ => format!("*.{} IN CNAME {}", domain, rule_str),
                };
                writeln!(&mut out, "{}", wc_record).unwrap();
            }
        }

        out
    }
}

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpNetwork {
    network: IpAddr,
    prefix_len: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid IP address or CIDR")]
pub struct InvalidIpNetwork;

impl FromStr for IpNetwork {
    type Err = InvalidIpNetwork;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let mut parts = value.split('/');
        let address = parts
            .next()
            .ok_or(InvalidIpNetwork)?
            .parse::<IpAddr>()
            .map_err(|_| InvalidIpNetwork)?;
        let provided_prefix = parts.next();
        if parts.next().is_some() {
            return Err(InvalidIpNetwork);
        }

        let max_prefix = match address {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        let prefix_len = match provided_prefix {
            Some(prefix) => prefix.parse::<u8>().map_err(|_| InvalidIpNetwork)?,
            None => max_prefix,
        };
        if prefix_len > max_prefix {
            return Err(InvalidIpNetwork);
        }

        let network = match address {
            IpAddr::V4(address) => {
                let mask = if prefix_len == 0 {
                    0
                } else {
                    u32::MAX << (32 - prefix_len)
                };
                IpAddr::V4(Ipv4Addr::from(u32::from(address) & mask))
            }
            IpAddr::V6(address) => {
                let mask = if prefix_len == 0 {
                    0
                } else {
                    u128::MAX << (128 - prefix_len)
                };
                IpAddr::V6(Ipv6Addr::from(u128::from(address) & mask))
            }
        };

        Ok(Self {
            network,
            prefix_len,
        })
    }
}

impl fmt::Display for IpNetwork {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let max_prefix = match self.network {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        };
        if self.prefix_len == max_prefix {
            write!(formatter, "{}", self.network)
        } else {
            write!(formatter, "{}/{}", self.network, self.prefix_len)
        }
    }
}

impl Serialize for IpNetwork {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for IpNetwork {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let value = String::deserialize(deserializer)?;
        value.parse().map_err(serde::de::Error::custom)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Protocol {
    Tcp,
    Udp,
    Icmp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Action {
    Allow,
    Deny,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PortRange {
    pub start: u16,
    pub end: u16,
}

impl PortRange {
    pub fn single(port: u16) -> Self {
        Self {
            start: port,
            end: port,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirewallRule {
    pub name: String,
    pub action: Action,
    pub protocol: Protocol,
    pub port_range: Option<PortRange>,
    pub source_ip: Option<IpNetwork>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirewallPolicy {
    pub rules: Vec<FirewallRule>,
    pub default_action: Action,
}

#[derive(Debug, thiserror::Error)]
pub enum FirewallError {
    #[error("Emergency access (SSH on port 22) is not explicitly allowed in the rules")]
    LockoutPreventionFailed,
    #[error("Invalid port range: start {0} > end {1}")]
    InvalidPortRange(u16, u16),
}

impl FirewallPolicy {
    pub fn validate(&self) -> Result<(), FirewallError> {
        // Validation 1: Lockout prevention
        // We must ensure there is an Allow rule for TCP port 22, or default is Allow (but usually default is Deny)
        let mut ssh_allowed = false;

        for rule in &self.rules {
            if let Some(ref pr) = rule.port_range {
                if pr.start > pr.end {
                    return Err(FirewallError::InvalidPortRange(pr.start, pr.end));
                }
            }

            if rule.action == Action::Allow && rule.protocol == Protocol::Tcp {
                match rule.port_range {
                    Some(ref pr) if pr.start <= 22 && pr.end >= 22 => ssh_allowed = true,
                    None => ssh_allowed = true, // 'None' port range implies ALL ports allowed
                    _ => {}
                }
            }
        }

        if !ssh_allowed && self.default_action == Action::Deny {
            return Err(FirewallError::LockoutPreventionFailed);
        }

        Ok(())
    }

    pub fn generate_apply_plan(&self) -> Result<String, FirewallError> {
        self.validate()?;

        // Generate a pseudo-config / plan (e.g. ufw or iptables representation)
        let mut plan = String::new();
        plan.push_str(&format!("# Firewall Apply Plan\n"));
        plan.push_str(&format!("DEFAULT_ACTION={:?}\n", self.default_action));
        for rule in &self.rules {
            let port = if let Some(ref p) = rule.port_range {
                format!("{}:{}", p.start, p.end)
            } else {
                "ANY".to_string()
            };
            let source = rule
                .source_ip
                .as_ref()
                .map(ToString::to_string)
                .unwrap_or_else(|| "ANY".to_string());
            plan.push_str(&format!(
                "RULE: {:?} {:?} PORT {} SOURCE {}\n",
                rule.action, rule.protocol, port, source
            ));
        }

        Ok(plan)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_valid_policy_with_ssh() {
        let policy = FirewallPolicy {
            rules: vec![FirewallRule {
                name: "Allow SSH".into(),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange::single(22)),
                source_ip: None,
            }],
            default_action: Action::Deny,
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_lockout_prevention() {
        let policy = FirewallPolicy {
            rules: vec![FirewallRule {
                name: "Allow HTTP".into(),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange::single(80)),
                source_ip: None,
            }],
            default_action: Action::Deny,
        };
        let res = policy.validate();
        assert!(matches!(res, Err(FirewallError::LockoutPreventionFailed)));
    }

    #[test]
    fn test_invalid_port_range() {
        let policy = FirewallPolicy {
            rules: vec![FirewallRule {
                name: "Invalid".into(),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange {
                    start: 100,
                    end: 50,
                }),
                source_ip: None,
            }],
            default_action: Action::Allow,
        };
        let res = policy.validate();
        assert!(matches!(res, Err(FirewallError::InvalidPortRange(100, 50))));
    }

    #[test]
    fn canonicalizes_firewall_source_networks_before_rendering() {
        let policy = FirewallPolicy {
            rules: vec![FirewallRule {
                name: "Allow SSH".into(),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange::single(22)),
                source_ip: Some("192.0.2.127/24".parse().unwrap()),
            }],
            default_action: Action::Deny,
        };

        assert!(policy
            .generate_apply_plan()
            .unwrap()
            .contains("SOURCE 192.0.2.0/24"));
    }

    #[test]
    fn rejects_firewall_source_injection_and_invalid_cidr() {
        for value in [
            "192.0.2.1\nDEFAULT_ACTION=Allow",
            "192.0.2.1/33",
            "2001:db8::1/129",
            "192.0.2.1/24/1",
        ] {
            assert!(value.parse::<IpNetwork>().is_err(), "accepted {value:?}");
        }
    }

    #[test]
    fn serde_rejects_firewall_source_injection() {
        let value = r#""192.0.2.1\nDEFAULT_ACTION=Allow""#;
        assert!(serde_json::from_str::<IpNetwork>(value).is_err());
    }
}

#[test]
fn test_lockout_prevention_allow_all_ports() {
    let policy = FirewallPolicy {
        rules: vec![FirewallRule {
            name: "Allow ALL TCP".into(),
            action: Action::Allow,
            protocol: Protocol::Tcp,
            port_range: None,
            source_ip: None,
        }],
        default_action: Action::Deny,
    };
    assert!(policy.validate().is_ok());
}

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
    #[error("SSH listen port must be greater than 0")]
    InvalidSshListenPort,
    #[error("Emergency access (configured SSH port {0}) is not allowed by the rules")]
    LockoutPreventionFailed(u16),
    #[error("Invalid port range: start {0} > end {1}")]
    InvalidPortRange(u16, u16),
}

impl FirewallPolicy {
    /// Validate that firewall rules preserve access to the configured SSH port.
    ///
    /// Callers must pass the matching [`crate::ssh::SshHardeningConfig::listen_port`]
    /// so a non-default SSH port cannot be silently blocked by a port-22 check.
    pub fn validate(&self, ssh_listen_port: u16) -> Result<(), FirewallError> {
        if ssh_listen_port == 0 {
            return Err(FirewallError::InvalidSshListenPort);
        }

        let mut ssh_allowed = None;

        for rule in &self.rules {
            if let Some(ref pr) = rule.port_range {
                if pr.start > pr.end {
                    return Err(FirewallError::InvalidPortRange(pr.start, pr.end));
                }
            }

            let port_matches = match rule.port_range {
                Some(ref pr) => pr.start <= ssh_listen_port && pr.end >= ssh_listen_port,
                None => true,
            };

            let protocol_matches = rule.protocol == Protocol::Tcp;

            if port_matches && protocol_matches
                && ssh_allowed.is_none() {
                    match rule.action {
                        Action::Allow => ssh_allowed = Some(true),
                        Action::Deny => ssh_allowed = Some(false),
                    }
                }
        }

        let is_allowed = ssh_allowed.unwrap_or(self.default_action == Action::Allow);

        if !is_allowed {
            return Err(FirewallError::LockoutPreventionFailed(ssh_listen_port));
        }

        Ok(())
    }

    pub fn generate_apply_plan(&self, ssh_listen_port: u16) -> Result<String, FirewallError> {
        self.validate(ssh_listen_port)?;

        // Generate a pseudo-config / plan (e.g. ufw or iptables representation)
        let mut plan = String::new();
        plan.push_str("# Firewall Apply Plan\n");
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
        assert!(policy.validate(22).is_ok());
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
        let res = policy.validate(22);
        assert!(matches!(
            res,
            Err(FirewallError::LockoutPreventionFailed(22))
        ));
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
        let res = policy.validate(22);
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
            .generate_apply_plan(22)
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

    #[test]
    fn configured_ssh_port_must_be_allowed_instead_of_assuming_port_22() {
        let policy = FirewallPolicy {
            rules: vec![FirewallRule {
                name: "Allow default SSH".into(),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange::single(22)),
                source_ip: None,
            }],
            default_action: Action::Deny,
        };

        assert!(matches!(
            policy.validate(2222),
            Err(FirewallError::LockoutPreventionFailed(2222))
        ));
        assert!(matches!(
            policy.generate_apply_plan(2222),
            Err(FirewallError::LockoutPreventionFailed(2222))
        ));
    }

    #[test]
    fn configured_ssh_port_and_firewall_rule_preserve_emergency_access() {
        let ssh = crate::ssh::SshHardeningConfig {
            listen_port: 2222,
            ..Default::default()
        };
        let policy = FirewallPolicy {
            rules: vec![FirewallRule {
                name: "Allow configured SSH".into(),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange::single(ssh.listen_port)),
                source_ip: None,
            }],
            default_action: Action::Deny,
        };

        let plan = policy.generate_apply_plan(ssh.listen_port).unwrap();
        assert!(plan.contains("PORT 2222:2222"));
    }

    #[test]
    fn rejects_zero_configured_ssh_port() {
        let policy = FirewallPolicy {
            rules: vec![],
            default_action: Action::Allow,
        };

        assert!(matches!(
            policy.validate(0),
            Err(FirewallError::InvalidSshListenPort)
        ));
    }

    #[test]
    fn test_lockout_prevention_default_allow_with_deny_ssh() {
        let policy = FirewallPolicy {
            rules: vec![FirewallRule {
                name: "Deny SSH".into(),
                action: Action::Deny,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange::single(22)),
                source_ip: None,
            }],
            default_action: Action::Allow,
        };
        assert!(matches!(
            policy.validate(22),
            Err(FirewallError::LockoutPreventionFailed(22))
        ));
    }

    #[test]
    fn test_lockout_prevention_default_deny_with_deny_all_before_allow_ssh() {
        let policy = FirewallPolicy {
            rules: vec![
                FirewallRule {
                    name: "Deny All".into(),
                    action: Action::Deny,
                    protocol: Protocol::Tcp,
                    port_range: None,
                    source_ip: None,
                },
                FirewallRule {
                    name: "Allow SSH".into(),
                    action: Action::Allow,
                    protocol: Protocol::Tcp,
                    port_range: Some(PortRange::single(22)),
                    source_ip: None,
                },
            ],
            default_action: Action::Deny,
        };
        assert!(matches!(
            policy.validate(22),
            Err(FirewallError::LockoutPreventionFailed(22))
        ));
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
    assert!(policy.validate(22).is_ok());
}

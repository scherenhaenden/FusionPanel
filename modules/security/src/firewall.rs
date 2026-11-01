use serde::{Deserialize, Serialize};

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
        Self { start: port, end: port }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FirewallRule {
    pub name: String,
    pub action: Action,
    pub protocol: Protocol,
    pub port_range: Option<PortRange>,
    pub source_ip: Option<String>,
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
            let source = rule.source_ip.clone().unwrap_or_else(|| "ANY".to_string());
            plan.push_str(&format!("RULE: {:?} {:?} PORT {} SOURCE {}\n", rule.action, rule.protocol, port, source));
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
            rules: vec![
                FirewallRule {
                    name: "Allow SSH".into(),
                    action: Action::Allow,
                    protocol: Protocol::Tcp,
                    port_range: Some(PortRange::single(22)),
                    source_ip: None,
                }
            ],
            default_action: Action::Deny,
        };
        assert!(policy.validate().is_ok());
    }

    #[test]
    fn test_lockout_prevention() {
        let policy = FirewallPolicy {
            rules: vec![
                FirewallRule {
                    name: "Allow HTTP".into(),
                    action: Action::Allow,
                    protocol: Protocol::Tcp,
                    port_range: Some(PortRange::single(80)),
                    source_ip: None,
                }
            ],
            default_action: Action::Deny,
        };
        let res = policy.validate();
        assert!(matches!(res, Err(FirewallError::LockoutPreventionFailed)));
    }

    #[test]
    fn test_invalid_port_range() {
        let policy = FirewallPolicy {
            rules: vec![
                FirewallRule {
                    name: "Invalid".into(),
                    action: Action::Allow,
                    protocol: Protocol::Tcp,
                    port_range: Some(PortRange { start: 100, end: 50 }),
                    source_ip: None,
                }
            ],
            default_action: Action::Allow,
        };
        let res = policy.validate();
        assert!(matches!(res, Err(FirewallError::InvalidPortRange(100, 50))));
    }
}

#[test]
fn test_lockout_prevention_allow_all_ports() {
    let policy = FirewallPolicy {
        rules: vec![
            FirewallRule {
                name: "Allow ALL TCP".into(),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: None,
                source_ip: None,
            }
        ],
        default_action: Action::Deny,
    };
    assert!(policy.validate().is_ok());
}

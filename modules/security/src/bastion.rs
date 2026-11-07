#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::SystemTime;
use std::time::Duration;

use crate::firewall::{Action, FirewallPolicy, FirewallRule, IpNetwork, PortRange, Protocol};
use std::str::FromStr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BastionConfig {
    pub default_lease_duration: Duration,
    pub max_active_leases: usize,
    pub ssh_port: u16,
    pub rule_name_prefix: String,
}

impl Default for BastionConfig {
    fn default() -> Self {
        Self {
            default_lease_duration: Duration::from_secs(3600), // 1 hour
            max_active_leases: 100,
            ssh_port: 22,
            rule_name_prefix: "bastion-allow-".to_string(),
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum BastionError {
    #[error("Maximum active leases reached")]
    MaxLeasesReached,
    #[error("Invalid IP network provided")]
    InvalidIpNetwork,
}

#[derive(Debug, Clone)]
pub struct Lease {
    pub ip: IpAddr,
    pub expires_at: SystemTime,
}

pub struct BastionManager {
    config: BastionConfig,
    leases: HashMap<IpAddr, Lease>,
}

impl BastionManager {
    pub fn new(config: BastionConfig) -> Self {
        Self {
            config,
            leases: HashMap::new(),
        }
    }

    pub fn add_lease(&mut self, ip: IpAddr, now: SystemTime) -> Result<(), BastionError> {
        self.cleanup(now);

        if !self.leases.contains_key(&ip) && self.leases.len() >= self.config.max_active_leases {
            return Err(BastionError::MaxLeasesReached);
        }

        let expires_at = now + self.config.default_lease_duration;
        self.leases.insert(ip, Lease { ip, expires_at });

        Ok(())
    }

    pub fn remove_lease(&mut self, ip: &IpAddr) {
        self.leases.remove(ip);
    }

    pub fn cleanup(&mut self, now: SystemTime) {
        self.leases.retain(|_, lease| lease.expires_at > now);
    }

    pub fn get_active_leases(&self, now: SystemTime) -> Vec<Lease> {
        self.leases
            .values()
            .filter(|lease| lease.expires_at > now)
            .cloned()
            .collect()
    }

    pub fn sync_firewall(&mut self, policy: &mut FirewallPolicy, now: SystemTime) -> Result<(), BastionError> {
        self.cleanup(now);

        // Remove all previous bastion rules
        policy.rules.retain(|rule| !rule.name.starts_with(&self.config.rule_name_prefix));

        let mut new_rules = Vec::with_capacity(self.leases.len() + policy.rules.len());

        // Generate rules for active leases
        for (ip, _) in self.leases.iter() {
            let network = format!("{}/{}", ip, if ip.is_ipv4() { 32 } else { 128 });
            let ip_network = IpNetwork::from_str(&network).map_err(|_| BastionError::InvalidIpNetwork)?;

            let rule = FirewallRule {
                name: format!("{}{}", self.config.rule_name_prefix, ip),
                action: Action::Allow,
                protocol: Protocol::Tcp,
                port_range: Some(PortRange::single(self.config.ssh_port)),
                source_ip: Some(ip_network),
            };

            new_rules.push(rule);
        }

        new_rules.append(&mut policy.rules);
        policy.rules = new_rules;

        Ok(())
    }
}

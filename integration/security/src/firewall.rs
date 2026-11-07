#![forbid(unsafe_code)]

use ipnet::IpNet;
use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirewallRule {
    pub cidr: String,
    pub action: RuleAction,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum RuleAction {
    Allow,
    Deny,
}

#[derive(Debug, Clone)]
pub struct Firewall {
    rules: Arc<RwLock<Vec<FirewallRule>>>,
}

impl Default for Firewall {
    fn default() -> Self {
        Self::new()
    }
}

impl Firewall {
    pub fn new() -> Self {
        Self {
            rules: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn add_rule(&self, rule: FirewallRule) -> Result<(), String> {
        // Validate CIDR format
        if rule.cidr.parse::<IpNet>().is_err() {
            return Err("Invalid CIDR format".to_string());
        }

        let mut rules = self.rules.write().await;
        rules.push(rule);
        Ok(())
    }

    pub async fn get_rules(&self) -> Vec<FirewallRule> {
        let rules = self.rules.read().await;
        rules.clone()
    }

    pub async fn check_ip(&self, ip: IpAddr) -> RuleAction {
        let rules = self.rules.read().await;
        for rule in rules.iter() {
            if let Ok(net) = rule.cidr.parse::<IpNet>() {
                if net.contains(&ip) {
                    return rule.action.clone();
                }
            }
        }
        RuleAction::Allow // Default action
    }
}

#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Allow,
    Block403,
    Redirect(String),
}

impl fmt::Display for Action {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Action::Allow => write!(f, "yes"),
            Action::Block403 => write!(f, "no"),
            Action::Redirect(url) => write!(f, "redirect:{}", url),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuleType {
    Allowlist(Vec<String>),
    Blocklist(Vec<String>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeoIpFilterConfig {
    pub rule_type: RuleType,
    pub bypass_private_ips: bool,
    pub custom_action: Action,
}

pub struct GeoIpRuleBuilder {
    config: GeoIpFilterConfig,
}

impl GeoIpRuleBuilder {
    pub fn new(config: GeoIpFilterConfig) -> Self {
        Self { config }
    }

    pub fn build_nginx_map(&self) -> String {
        let mut map_block = String::new();
        map_block.push_str("map $geoip2_data_country_code $allowed_country {\n");
        map_block.push_str("    default ");

        match &self.config.rule_type {
            RuleType::Allowlist(_) => {
                map_block.push_str(&format!("{};\n", self.config.custom_action));
            }
            RuleType::Blocklist(_) => {
                map_block.push_str(&format!("{};\n", Action::Allow));
            }
        }

        let action_for_listed = match &self.config.rule_type {
            RuleType::Allowlist(_) => Action::Allow,
            RuleType::Blocklist(_) => self.config.custom_action.clone(),
        };

        let countries = match &self.config.rule_type {
            RuleType::Allowlist(c) => c,
            RuleType::Blocklist(c) => c,
        };

        for country in countries {
            map_block.push_str(&format!("    {} {};\n", country, action_for_listed));
        }

        map_block.push_str("}\n");
        map_block
    }

    pub fn build_bypass_map(&self) -> Option<String> {
        if !self.config.bypass_private_ips {
            return None;
        }

        let mut map_block = String::new();
        map_block.push_str("geo $bypass_geoip {\n");
        map_block.push_str("    default 0;\n");
        map_block.push_str("    10.0.0.0/8 1;\n");
        map_block.push_str("    172.16.0.0/12 1;\n");
        map_block.push_str("    192.168.0.0/16 1;\n");
        map_block.push_str("    127.0.0.1 1;\n");
        map_block.push_str("    ::1 1;\n");
        map_block.push_str("}\n");
        Some(map_block)
    }
}

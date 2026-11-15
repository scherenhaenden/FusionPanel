#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt::Write;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PostscreenError {
    #[error("DNSBL threshold must be greater than 0")]
    InvalidThreshold,
    #[error("DNSBL list name cannot be empty")]
    InvalidDnsblName,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PostscreenAction {
    Enforce,
    Drop,
    Ignore,
}

impl std::fmt::Display for PostscreenAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PostscreenAction::Enforce => write!(f, "enforce"),
            PostscreenAction::Drop => write!(f, "drop"),
            PostscreenAction::Ignore => write!(f, "ignore"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct DnsblSite {
    pub name: String,
    pub weight: i32,
}

#[derive(Debug, Clone)]
pub struct PostscreenConfig {
    pub dnsbl_sites: Vec<DnsblSite>,
    pub dnsbl_threshold: i32,
    pub greet_action: PostscreenAction,
    pub hangup_action: PostscreenAction,
    pub pipelining_action: PostscreenAction,
    pub non_smtp_command_action: PostscreenAction,
    pub bare_newline_action: PostscreenAction,
}

#[derive(Default)]
pub struct PostscreenConfigBuilder {
    dnsbl_sites: Vec<DnsblSite>,
    dnsbl_threshold: i32,
    greet_action: Option<PostscreenAction>,
    hangup_action: Option<PostscreenAction>,
    pipelining_action: Option<PostscreenAction>,
    non_smtp_command_action: Option<PostscreenAction>,
    bare_newline_action: Option<PostscreenAction>,
}

impl PostscreenConfigBuilder {
    pub fn new() -> Self {
        Self {
            dnsbl_sites: Vec::new(),
            dnsbl_threshold: 1, // Default to 1
            greet_action: None,
            hangup_action: None,
            pipelining_action: None,
            non_smtp_command_action: None,
            bare_newline_action: None,
        }
    }

    pub fn add_dnsbl_site(mut self, name: &str, weight: i32) -> Self {
        self.dnsbl_sites.push(DnsblSite {
            name: name.to_string(),
            weight,
        });
        self
    }

    pub fn dnsbl_threshold(mut self, threshold: i32) -> Self {
        self.dnsbl_threshold = threshold;
        self
    }

    pub fn greet_action(mut self, action: PostscreenAction) -> Self {
        self.greet_action = Some(action);
        self
    }

    pub fn hangup_action(mut self, action: PostscreenAction) -> Self {
        self.hangup_action = Some(action);
        self
    }

    pub fn pipelining_action(mut self, action: PostscreenAction) -> Self {
        self.pipelining_action = Some(action);
        self
    }

    pub fn non_smtp_command_action(mut self, action: PostscreenAction) -> Self {
        self.non_smtp_command_action = Some(action);
        self
    }

    pub fn bare_newline_action(mut self, action: PostscreenAction) -> Self {
        self.bare_newline_action = Some(action);
        self
    }

    pub fn build(self) -> Result<PostscreenConfig, PostscreenError> {
        if self.dnsbl_threshold <= 0 {
            return Err(PostscreenError::InvalidThreshold);
        }

        for site in &self.dnsbl_sites {
            if site.name.trim().is_empty() {
                return Err(PostscreenError::InvalidDnsblName);
            }
        }

        Ok(PostscreenConfig {
            dnsbl_sites: self.dnsbl_sites,
            dnsbl_threshold: self.dnsbl_threshold,
            greet_action: self.greet_action.unwrap_or(PostscreenAction::Enforce),
            hangup_action: self.hangup_action.unwrap_or(PostscreenAction::Enforce),
            pipelining_action: self.pipelining_action.unwrap_or(PostscreenAction::Enforce),
            non_smtp_command_action: self.non_smtp_command_action.unwrap_or(PostscreenAction::Drop),
            bare_newline_action: self.bare_newline_action.unwrap_or(PostscreenAction::Ignore),
        })
    }
}

impl PostscreenConfig {
    pub fn generate_main_cf(&self) -> String {
        let mut cf = String::new();

        let _ = writeln!(cf, "postscreen_greet_action = {}", self.greet_action);
        let _ = writeln!(cf, "postscreen_bare_newline_action = {}", self.bare_newline_action);
        let _ = writeln!(cf, "postscreen_non_smtp_command_action = {}", self.non_smtp_command_action);
        let _ = writeln!(cf, "postscreen_pipelining_action = {}", self.pipelining_action);
        let _ = writeln!(cf, "postscreen_hangup_action = {}", self.hangup_action);

        if !self.dnsbl_sites.is_empty() {
            let sites = self.dnsbl_sites
                .iter()
                .map(|site| format!("{}*{}", site.name, site.weight))
                .collect::<Vec<_>>()
                .join(", ");

            let _ = writeln!(cf, "postscreen_dnsbl_sites = {}", sites);
            let _ = writeln!(cf, "postscreen_dnsbl_threshold = {}", self.dnsbl_threshold);
            let _ = writeln!(cf, "postscreen_dnsbl_action = enforce");
        }

        cf
    }

    pub fn generate_master_cf(&self) -> String {
        "smtp      inet  n       -       y       -       1       postscreen\n\
smtpd     pass  -       -       y       -       -       smtpd\n\
tlsproxy  unix  -       -       y       -       0       tlsproxy\n\
dnsblog   unix  -       -       y       -       0       dnsblog\n".to_string()
    }

    pub fn calculate_score(&self, query_responses: &HashMap<String, Vec<String>>) -> i32 {
        let mut total_score = 0;
        for site in &self.dnsbl_sites {
            if let Some(responses) = query_responses.get(&site.name) {
                // Typical DNSBL lists return a 127.x.x.x code if listed.
                // NXDOMAIN is translated to missing/empty list.
                if !responses.is_empty() {
                    total_score += site.weight;
                }
            }
        }
        total_score
    }

    pub fn is_rejected(&self, query_responses: &HashMap<String, Vec<String>>) -> bool {
        self.calculate_score(query_responses) >= self.dnsbl_threshold
    }
}

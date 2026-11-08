#![forbid(unsafe_code)]

use crate::domain::error::{DomainError, Result};
use std::fmt::Write;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamServer {
    pub address: String, // could be IP:port, domain:port, or unix socket
    pub max_fails: Option<u32>,
    pub fail_timeout: Option<String>,
    pub backup: bool,
    pub weight: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchBlock {
    pub name: String,
    pub statuses: Vec<String>,
    pub body_regex: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveHealthCheck {
    pub interval: Option<String>,
    pub passes: Option<u32>,
    pub fails: Option<u32>,
    pub uri: Option<String>,
    pub match_name: Option<String>,
    pub port: Option<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpstreamPoolConfig {
    pub name: String,
    pub servers: Vec<UpstreamServer>,
    pub keepalive: Option<u32>,
    pub zone_name: Option<String>,
    pub zone_size: Option<String>,
    pub active_check: Option<ActiveHealthCheck>,
    pub match_blocks: Vec<MatchBlock>,
}

pub fn generate_upstream_pool(config: &UpstreamPoolConfig) -> Result<String> {
    if config.name.is_empty() {
        return Err(DomainError::InvalidInput(
            "Upstream pool name cannot be empty".into(),
        ));
    }
    if !config
        .name
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
    {
        return Err(DomainError::InvalidInput(
            "Invalid characters in upstream pool name".into(),
        ));
    }
    if config.servers.is_empty() {
        return Err(DomainError::InvalidInput(
            "Upstream pool must contain at least one server".into(),
        ));
    }

    let mut out = String::new();
    writeln!(&mut out, "upstream {} {{", config.name).unwrap();

    if let Some(zone_name) = &config.zone_name {
        if !zone_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(DomainError::InvalidInput(
                "Invalid characters in zone name".into(),
            ));
        }
        let size = config.zone_size.as_deref().unwrap_or("64k");
        if !size
            .chars()
            .all(|c| c.is_ascii_digit() || c == 'k' || c == 'm' || c == 'K' || c == 'M')
        {
            return Err(DomainError::InvalidInput("Invalid zone size format".into()));
        }
        writeln!(&mut out, "    zone {} {};", zone_name, size).unwrap();
    }

    for server in &config.servers {
        if server.address.is_empty() {
            return Err(DomainError::InvalidInput(
                "Upstream server address cannot be empty".into(),
            ));
        }

        if server.address.contains(|c| {
            c == ';'
                || c == '\n'
                || c == '\r'
                || c == '{'
                || c == '}'
                || c == '$'
                || c == '"'
                || c == '\''
                || c == '\\'
        }) {
            return Err(DomainError::InvalidInput(
                "Invalid characters in upstream server address".into(),
            ));
        }

        write!(&mut out, "    server {}", server.address).unwrap();

        if let Some(weight) = server.weight {
            write!(&mut out, " weight={}", weight).unwrap();
        }

        if let Some(max_fails) = server.max_fails {
            write!(&mut out, " max_fails={}", max_fails).unwrap();
        }

        if let Some(fail_timeout) = &server.fail_timeout {
            if !fail_timeout
                .chars()
                .all(|c| c.is_ascii_digit() || c == 's' || c == 'm' || c == 'h')
            {
                return Err(DomainError::InvalidInput(
                    "Invalid fail_timeout format".into(),
                ));
            }
            write!(&mut out, " fail_timeout={}", fail_timeout).unwrap();
        }

        if server.backup {
            write!(&mut out, " backup").unwrap();
        }

        writeln!(&mut out, ";").unwrap();
    }

    if let Some(keepalive) = config.keepalive {
        writeln!(&mut out, "    keepalive {};", keepalive).unwrap();
    }

    writeln!(&mut out, "}}").unwrap();

    for mb in &config.match_blocks {
        if !mb
            .name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(DomainError::InvalidInput("Invalid match block name".into()));
        }
        writeln!(&mut out, "\nmatch {} {{", mb.name).unwrap();
        if !mb.statuses.is_empty() {
            for status in &mb.statuses {
                if status.contains(|c| c == ';' || c == '\n' || c == '\r' || c == '{' || c == '}') {
                    return Err(DomainError::InvalidInput(
                        "Invalid characters in status match".into(),
                    ));
                }
            }
            writeln!(&mut out, "    status {};", mb.statuses.join(" ")).unwrap();
        }
        if let Some(body) = &mb.body_regex {
            if body.contains(|c| c == '\n' || c == '\r' || c == ';') {
                return Err(DomainError::InvalidInput(
                    "Invalid characters in body regex".into(),
                ));
            }
            writeln!(&mut out, "    body ~ \"{}\";", body.replace('"', "\\\"")).unwrap();
            // Escape quotes
        }
        writeln!(&mut out, "}}").unwrap();
    }

    Ok(out)
}

/// Generates the `health_check` directive for a location block
pub fn generate_health_check_directive(check: &ActiveHealthCheck) -> Result<String> {
    let mut out = "health_check".to_string();

    if let Some(interval) = &check.interval {
        if !interval
            .chars()
            .all(|c| c.is_ascii_digit() || c == 's' || c == 'm' || c == 'h')
        {
            return Err(DomainError::InvalidInput(
                "Invalid health check interval".into(),
            ));
        }
        out.push_str(&format!(" interval={}", interval));
    }

    if let Some(fails) = check.fails {
        out.push_str(&format!(" fails={}", fails));
    }

    if let Some(passes) = check.passes {
        out.push_str(&format!(" passes={}", passes));
    }

    if let Some(uri) = &check.uri {
        if uri.contains(|c| {
            c == ';'
                || c == '\n'
                || c == '\r'
                || c == '{'
                || c == '}'
                || c == '"'
                || c == '\''
                || c == '$'
        }) {
            return Err(DomainError::InvalidInput(
                "Invalid characters in health check URI".into(),
            ));
        }
        out.push_str(&format!(" uri={}", uri));
    }

    if let Some(match_name) = &check.match_name {
        if !match_name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
        {
            return Err(DomainError::InvalidInput("Invalid match name".into()));
        }
        out.push_str(&format!(" match={}", match_name));
    }

    if let Some(port) = check.port {
        out.push_str(&format!(" port={}", port));
    }

    out.push(';');
    Ok(out)
}

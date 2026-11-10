#![forbid(unsafe_code)]

use std::net::IpAddr;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum DotError {
    #[error("Invalid SPKI Pin format: {0}")]
    InvalidSpkiPin(String),
    #[error("Missing TLS hostname for strict verification")]
    MissingTlsHostname,
    #[error("Unsupported protocol: {0}")]
    UnsupportedProtocol(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protocol {
    DoT, // DNS-over-TLS (Port 853)
    DoH, // DNS-over-HTTPS (Port 443)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpkiPin(String);

impl SpkiPin {
    pub fn new(pin: &str) -> Result<Self, DotError> {
        if !pin.starts_with("pin-sha256=") {
            return Err(DotError::InvalidSpkiPin("Must start with pin-sha256=".to_string()));
        }
        let b64 = &pin["pin-sha256=".len()..];
        if b64.is_empty() || !b64.chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=') {
            return Err(DotError::InvalidSpkiPin("Invalid base64 payload".to_string()));
        }
        Ok(SpkiPin(pin.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FallbackPolicy {
    Strict,
    Opportunistic,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FallbackDecision {
    pub allow_plaintext: bool,
    pub ignore_cert_errors: bool,
}

impl FallbackPolicy {
    pub fn evaluate(&self) -> FallbackDecision {
        match self {
            FallbackPolicy::Strict => FallbackDecision {
                allow_plaintext: false,
                ignore_cert_errors: false,
            },
            FallbackPolicy::Opportunistic => FallbackDecision {
                allow_plaintext: true,
                ignore_cert_errors: true,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthState {
    pub is_healthy: bool,
    pub latency_ms: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpstreamEndpoint {
    pub ip: IpAddr,
    pub port: u16,
    pub protocol: Protocol,
    pub tls_auth_name: Option<String>,
    pub spki_pin: Option<SpkiPin>,
    pub health: HealthState,
}

impl UpstreamEndpoint {
    pub fn new(
        ip: IpAddr,
        port: u16,
        protocol: Protocol,
        tls_auth_name: Option<String>,
        spki_pin: Option<SpkiPin>,
    ) -> Result<Self, DotError> {
        Ok(Self {
            ip,
            port,
            protocol,
            tls_auth_name,
            spki_pin,
            health: HealthState {
                is_healthy: true,
                latency_ms: None,
            },
        })
    }

    pub fn update_health(&mut self, is_healthy: bool, latency_ms: Option<u32>) {
        self.health.is_healthy = is_healthy;
        self.health.latency_ms = latency_ms;
    }
}

#[derive(Debug, Clone)]
pub struct DotForwarderPolicy {
    pub endpoints: Vec<UpstreamEndpoint>,
    pub fallback_policy: FallbackPolicy,
}

impl DotForwarderPolicy {
    pub fn render_coredns(&self) -> Result<String, DotError> {
        if self.endpoints.is_empty() {
            return Ok(String::new());
        }

        if self.fallback_policy == FallbackPolicy::Strict {
            for ep in &self.endpoints {
                if ep.tls_auth_name.is_none() {
                    return Err(DotError::MissingTlsHostname);
                }
            }
        }

        let mut config = String::from("forward .");

        for ep in &self.endpoints {
            match ep.protocol {
                Protocol::DoT => config.push_str(&format!(" tls://{}:{}", ep.ip, ep.port)),
                Protocol::DoH => config.push_str(&format!(" https://{}:{}", ep.ip, ep.port)),
            }
        }

        config.push_str(" {\n");

        match self.fallback_policy {
            FallbackPolicy::Strict => {
                let hostnames: Vec<_> = self.endpoints.iter().filter_map(|e| e.tls_auth_name.as_ref()).map(|s| s.as_str()).collect();
                if !hostnames.is_empty() {
                    // Extract unique hostnames
                    let mut unique = hostnames;
                    unique.dedup();
                    config.push_str(&format!("    tls_servername {}\n", unique[0])); // Simply take the first for the snippet
                }
            },
            FallbackPolicy::Opportunistic => {
                config.push_str("    tls_servername ignore\n"); // opportunistic fallback
            }
        }

        config.push_str("}\n");

        Ok(config)
    }

    pub fn render_unbound(&self) -> Result<String, DotError> {
        if self.endpoints.is_empty() {
            return Ok(String::new());
        }

        if self.fallback_policy == FallbackPolicy::Strict {
            for ep in &self.endpoints {
                if ep.tls_auth_name.is_none() {
                    return Err(DotError::MissingTlsHostname);
                }
            }
        }

        let mut config = String::new();
        config.push_str("forward-zone:\n");
        config.push_str("    name: \".\"\n");

        let decision = self.fallback_policy.evaluate();

        for ep in &self.endpoints {
            match ep.protocol {
                Protocol::DoT => {
                    let mut line = format!("    forward-addr: {}@{}", ep.ip, ep.port);
                    if !decision.ignore_cert_errors {
                        if let Some(name) = &ep.tls_auth_name {
                            line.push_str(&format!("#{}", name));
                        }
                    }
                    config.push_str(&line);
                    config.push('\n');
                },
                Protocol::DoH => {
                    return Err(DotError::UnsupportedProtocol("Unbound configuration snippet does not support DoH directly".into()));
                }
            }
        }

        if !decision.allow_plaintext {
            config.push_str("    forward-tls-upstream: yes\n");
        } else {
            config.push_str("    forward-tls-upstream: no\n");
        }

        Ok(config)
    }
}

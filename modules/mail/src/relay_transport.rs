#![forbid(unsafe_code)]

use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportMethod {
    Smtp,
    Lmtp,
    Local,
    Error(String),
    Discard,
    Custom(String),
}

impl fmt::Display for TransportMethod {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TransportMethod::Smtp => write!(f, "smtp"),
            TransportMethod::Lmtp => write!(f, "lmtp"),
            TransportMethod::Local => write!(f, "local"),
            TransportMethod::Error(msg) => write!(f, "error:{}", msg),
            TransportMethod::Discard => write!(f, "discard"),
            TransportMethod::Custom(s) => write!(f, "{}", s),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelayHost {
    pub host: String,
    pub port: Option<u16>,
    pub mx_lookup: bool,
}

impl RelayHost {
    pub fn new(host: impl Into<String>, port: Option<u16>, mx_lookup: bool) -> Self {
        Self {
            host: host.into(),
            port,
            mx_lookup,
        }
    }
}

impl fmt::Display for RelayHost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let host_part = if self.mx_lookup {
            self.host.clone()
        } else {
            format!("[{}]", self.host)
        };

        if let Some(port) = self.port {
            write!(f, "{}:{}", host_part, port)
        } else {
            write!(f, "{}", host_part)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransportEntry {
    pub pattern: String,
    pub method: TransportMethod,
    pub nexthop: Option<RelayHost>,
}

impl TransportEntry {
    pub fn new(
        pattern: impl Into<String>,
        method: TransportMethod,
        nexthop: Option<RelayHost>,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            method,
            nexthop,
        }
    }
}

impl fmt::Display for TransportEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref nexthop) = self.nexthop {
            write!(f, "{}\t{}:{}", self.pattern, self.method, nexthop)
        } else {
            write!(f, "{}\t{}", self.pattern, self.method)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenderRelayEntry {
    pub sender: String,
    pub relay: RelayHost,
}

impl SenderRelayEntry {
    pub fn new(sender: impl Into<String>, relay: RelayHost) -> Self {
        Self {
            sender: sender.into(),
            relay,
        }
    }
}

impl fmt::Display for SenderRelayEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\t{}", self.sender, self.relay)
    }
}

#[derive(Debug, Clone, Default)]
pub struct RelayConfig {
    pub transports: Vec<TransportEntry>,
    pub sender_relays: Vec<SenderRelayEntry>,
}

impl RelayConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_transport(&mut self, entry: TransportEntry) {
        self.transports.push(entry);
    }

    pub fn add_sender_relay(&mut self, entry: SenderRelayEntry) {
        self.sender_relays.push(entry);
    }

    pub fn generate_transport_map(&self) -> String {
        let mut lines = Vec::new();
        lines.push("# Generated Postfix transport map".to_string());
        for entry in &self.transports {
            lines.push(entry.to_string());
        }
        lines.join("\n") + "\n"
    }

    pub fn generate_sender_relay_map(&self) -> String {
        let mut lines = Vec::new();
        lines.push("# Generated Postfix sender-dependent relayhost map".to_string());
        for entry in &self.sender_relays {
            lines.push(entry.to_string());
        }
        lines.join("\n") + "\n"
    }
}

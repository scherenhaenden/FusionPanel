#![forbid(unsafe_code)]

use std::collections::HashMap;
use std::fmt::Write;
use thiserror::Error;

#[derive(Debug, Clone, Error, PartialEq, Eq)]
pub enum WireGuardError {
    #[error("Node not found in mesh: {0}")]
    NodeNotFound(String),
    #[error("Formatting error")]
    FormatError,
    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),
}

impl From<std::fmt::Error> for WireGuardError {
    fn from(_: std::fmt::Error) -> Self {
        WireGuardError::FormatError
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WireGuardNode {
    pub id: String,
    pub private_key: String,
    pub public_key: String,
    pub listen_port: Option<u16>,
    pub address: Option<String>,
    pub endpoint: Option<String>,
    pub allowed_ips: Vec<String>,
    pub persistent_keepalive: Option<u16>,
}

#[derive(Debug, Clone, Default)]
pub struct WireGuardMeshConfigurator {
    nodes: HashMap<String, WireGuardNode>,
    psks: HashMap<(String, String), String>,
    default_keepalive: Option<u16>,
}

impl WireGuardMeshConfigurator {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_default_keepalive(&mut self, keepalive: u16) {
        self.default_keepalive = Some(keepalive);
    }

    pub fn add_node(&mut self, node: WireGuardNode) -> Result<(), WireGuardError> {
        if node.id.is_empty() {
            return Err(WireGuardError::InvalidConfig("Node ID cannot be empty".into()));
        }
        if node.private_key.is_empty() || node.public_key.is_empty() {
            return Err(WireGuardError::InvalidConfig("Keys cannot be empty".into()));
        }
        self.nodes.insert(node.id.clone(), node);
        Ok(())
    }

    pub fn remove_node(&mut self, id: &str) {
        self.nodes.remove(id);
        self.psks.retain(|(a, b), _| a != id && b != id);
    }

    fn psk_key(a: &str, b: &str) -> (String, String) {
        if a < b {
            (a.to_string(), b.to_string())
        } else {
            (b.to_string(), a.to_string())
        }
    }

    pub fn set_psk(&mut self, node_a: &str, node_b: &str, psk: String) -> Result<(), WireGuardError> {
        if !self.nodes.contains_key(node_a) {
            return Err(WireGuardError::NodeNotFound(node_a.to_string()));
        }
        if !self.nodes.contains_key(node_b) {
            return Err(WireGuardError::NodeNotFound(node_b.to_string()));
        }
        if psk.is_empty() {
            return Err(WireGuardError::InvalidConfig("PSK cannot be empty".into()));
        }
        self.psks.insert(Self::psk_key(node_a, node_b), psk);
        Ok(())
    }

    pub fn get_node(&self, id: &str) -> Option<&WireGuardNode> {
        self.nodes.get(id)
    }

    pub fn generate_config(&self, target_node_id: &str) -> Result<String, WireGuardError> {
        let target_node = self.nodes.get(target_node_id)
            .ok_or_else(|| WireGuardError::NodeNotFound(target_node_id.to_string()))?;

        let mut config = String::new();

        writeln!(config, "[Interface]")?;
        writeln!(config, "PrivateKey = {}", target_node.private_key)?;
        if let Some(address) = &target_node.address {
            writeln!(config, "Address = {}", address)?;
        }
        if let Some(port) = target_node.listen_port {
            writeln!(config, "ListenPort = {}", port)?;
        }

        // Sort peers by ID to make output deterministic
        let mut peer_ids: Vec<&String> = self.nodes.keys().filter(|k| *k != target_node_id).collect();
        peer_ids.sort();

        for peer_id in peer_ids {
            let peer = self.nodes.get(peer_id).unwrap();
            writeln!(config, "\n[Peer]")?;
            writeln!(config, "PublicKey = {}", peer.public_key)?;

            if let Some(psk) = self.psks.get(&Self::psk_key(target_node_id, peer_id)) {
                writeln!(config, "PresharedKey = {}", psk)?;
            }

            if !peer.allowed_ips.is_empty() {
                writeln!(config, "AllowedIPs = {}", peer.allowed_ips.join(", "))?;
            }

            if let Some(endpoint) = &peer.endpoint {
                writeln!(config, "Endpoint = {}", endpoint)?;
            }

            let keepalive = peer.persistent_keepalive.or(self.default_keepalive);
            if let Some(keepalive) = keepalive {
                writeln!(config, "PersistentKeepalive = {}", keepalive)?;
            }
        }

        Ok(config)
    }
}

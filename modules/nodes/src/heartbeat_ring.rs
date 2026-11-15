#![forbid(unsafe_code)]

use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum HeartbeatRingError {
    #[error("Node {0} already exists in the ring")]
    NodeAlreadyExists(String),
    #[error("Node {0} not found in the ring")]
    NodeNotFound(String),
    #[error("Not enough nodes to form a ring")]
    InsufficientNodes,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeState {
    Active,
    Suspected,
    Dead,
}

#[derive(Debug, Clone)]
pub struct NodeInfo {
    pub id: String,
    pub sequence: u64,
    pub missed_heartbeats: u64,
    pub state: NodeState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuorumStatus {
    HasQuorum,
    IsolatedPartition,
}

#[derive(Debug, Clone)]
pub struct HeartbeatRing {
    nodes: BTreeMap<String, NodeInfo>,
    suspect_threshold: u64,
    dead_threshold: u64,
}

impl Default for HeartbeatRing {
    fn default() -> Self {
        Self::new(3, 10)
    }
}

impl HeartbeatRing {
    pub fn new(suspect_threshold: u64, dead_threshold: u64) -> Self {
        Self {
            nodes: BTreeMap::new(),
            suspect_threshold,
            dead_threshold,
        }
    }

    pub fn add_node(&mut self, id: String) -> Result<(), HeartbeatRingError> {
        if self.nodes.contains_key(&id) {
            return Err(HeartbeatRingError::NodeAlreadyExists(id));
        }
        self.nodes.insert(
            id.clone(),
            NodeInfo {
                id,
                sequence: 0,
                missed_heartbeats: 0,
                state: NodeState::Active,
            },
        );
        Ok(())
    }

    pub fn remove_node(&mut self, id: &str) -> Result<(), HeartbeatRingError> {
        if self.nodes.remove(id).is_none() {
            return Err(HeartbeatRingError::NodeNotFound(id.to_string()));
        }
        Ok(())
    }

    pub fn receive_heartbeat(&mut self, id: &str, sequence: u64) -> Result<(), HeartbeatRingError> {
        let node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| HeartbeatRingError::NodeNotFound(id.to_string()))?;
        if sequence >= node.sequence {
            node.sequence = sequence;
            node.missed_heartbeats = 0;
            node.state = NodeState::Active;
        }
        Ok(())
    }

    pub fn tick_node(&mut self, id: &str) -> Result<(), HeartbeatRingError> {
        let suspect = self.suspect_threshold;
        let dead = self.dead_threshold;

        let node = self
            .nodes
            .get_mut(id)
            .ok_or_else(|| HeartbeatRingError::NodeNotFound(id.to_string()))?;

        node.missed_heartbeats = node.missed_heartbeats.saturating_add(1);
        if node.missed_heartbeats >= dead {
            node.state = NodeState::Dead;
        } else if node.missed_heartbeats >= suspect {
            node.state = NodeState::Suspected;
        }
        Ok(())
    }

    pub fn tick_all(&mut self) {
        let suspect = self.suspect_threshold;
        let dead = self.dead_threshold;
        for node in self.nodes.values_mut() {
            node.missed_heartbeats = node.missed_heartbeats.saturating_add(1);
            if node.missed_heartbeats >= dead {
                node.state = NodeState::Dead;
            } else if node.missed_heartbeats >= suspect {
                node.state = NodeState::Suspected;
            }
        }
    }

    pub fn successor(&self, id: &str) -> Result<String, HeartbeatRingError> {
        if !self.nodes.contains_key(id) {
            return Err(HeartbeatRingError::NodeNotFound(id.to_string()));
        }

        let mut first_active_after = None;
        let mut first_active_overall = None;

        for (k, n) in self.nodes.iter() {
            if n.state != NodeState::Dead {
                if first_active_overall.is_none() {
                    first_active_overall = Some(k.clone());
                }
                if k.as_str() > id {
                    first_active_after = Some(k.clone());
                    break;
                }
            }
        }

        if let Some(next) = first_active_after {
            Ok(next)
        } else if let Some(first) = first_active_overall {
            if first == id {
                Err(HeartbeatRingError::InsufficientNodes)
            } else {
                Ok(first)
            }
        } else {
            Err(HeartbeatRingError::InsufficientNodes)
        }
    }

    pub fn check_quorum(&self) -> QuorumStatus {
        let total_nodes = self.nodes.len();
        if total_nodes == 0 {
            return QuorumStatus::IsolatedPartition;
        }

        let reachable_nodes = self
            .nodes
            .values()
            .filter(|n| n.state != NodeState::Dead)
            .count();

        if reachable_nodes > total_nodes / 2 {
            QuorumStatus::HasQuorum
        } else {
            QuorumStatus::IsolatedPartition
        }
    }

    pub fn get_node(&self, id: &str) -> Option<&NodeInfo> {
        self.nodes.get(id)
    }

    pub fn nodes(&self) -> impl Iterator<Item = &NodeInfo> {
        self.nodes.values()
    }
}

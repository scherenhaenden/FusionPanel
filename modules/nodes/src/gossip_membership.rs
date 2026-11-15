#![forbid(unsafe_code)]

use std::collections::HashMap;
use chrono::{DateTime, Utc};
use serde::{Serialize, Deserialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum NodeState {
    Alive,
    Suspect,
    Dead,
    Left,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipConfig {
    pub suspect_timeout_secs: i64,
    pub dead_timeout_secs: i64,
}

impl Default for GossipConfig {
    fn default() -> Self {
        Self {
            suspect_timeout_secs: 5,
            dead_timeout_secs: 30,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub id: String,
    pub state: NodeState,
    pub incarnation: u64,
    pub state_changed_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GossipMembership {
    pub config: GossipConfig,
    pub nodes: HashMap<String, NodeInfo>,
}

impl GossipMembership {
    pub fn new(config: GossipConfig) -> Self {
        Self {
            config,
            nodes: HashMap::new(),
        }
    }

    pub fn update_node(&mut self, id: String, new_state: NodeState, incarnation: u64, now: DateTime<Utc>) {
        if let Some(existing) = self.nodes.get_mut(&id) {
            // State precedence logic
            if incarnation > existing.incarnation {
                existing.incarnation = incarnation;
                if existing.state != new_state {
                    existing.state = new_state;
                    existing.state_changed_at = now;
                }
            } else if incarnation == existing.incarnation {
                // If same incarnation, transitions are allowed if they are "worse" or "final"
                if new_state > existing.state {
                    existing.state = new_state;
                    existing.state_changed_at = now;
                }
            }
        } else {
            self.nodes.insert(id.clone(), NodeInfo {
                id,
                state: new_state,
                incarnation,
                state_changed_at: now,
            });
        }
    }

    pub fn tick(&mut self, now: DateTime<Utc>) {
        for node in self.nodes.values_mut() {
            let duration = now.signed_duration_since(node.state_changed_at).num_seconds();

            match node.state {
                NodeState::Suspect => {
                    if duration >= self.config.suspect_timeout_secs {
                        node.state = NodeState::Dead;
                        node.state_changed_at = now;
                    }
                }
                NodeState::Dead => {
                    if duration >= self.config.dead_timeout_secs {
                        node.state = NodeState::Left;
                        node.state_changed_at = now;
                    }
                }
                _ => {}
            }
        }
    }

    pub fn get_node(&self, id: &str) -> Option<&NodeInfo> {
        self.nodes.get(id)
    }

    pub fn get_active_nodes(&self) -> Vec<&NodeInfo> {
        self.nodes.values().filter(|n| n.state == NodeState::Alive).collect()
    }
}

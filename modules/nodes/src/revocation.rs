use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevocationList {
    revoked_nodes: Vec<Uuid>,
}

impl Default for RevocationList {
    fn default() -> Self {
        Self::new()
    }
}

impl RevocationList {
    pub fn new() -> Self {
        Self {
            revoked_nodes: Vec::new(),
        }
    }

    pub fn add(&mut self, node_id: Uuid) {
        if !self.revoked_nodes.contains(&node_id) {
            self.revoked_nodes.push(node_id);
        }
    }

    pub fn is_revoked(&self, node_id: &Uuid) -> bool {
        self.revoked_nodes.contains(node_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_revocation_list() {
        let mut rl = RevocationList::new();
        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();

        rl.add(node1);

        assert!(rl.is_revoked(&node1));
        assert!(!rl.is_revoked(&node2));
    }
}

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeLoad {
    pub node_id: Uuid,
    pub cpu_usage: f64,
    pub memory_usage: f64,
    pub active_sites: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlacementConstraint {
    pub max_cpu: f64,
    pub max_memory: f64,
    pub max_sites: usize,
}

#[derive(thiserror::Error, Debug)]
pub enum PlacementError {
    #[error("No suitable node found")]
    NoSuitableNode,
}

pub fn plan_placement(
    nodes: &[NodeLoad],
    constraint: &PlacementConstraint,
) -> Result<Uuid, PlacementError> {
    let mut best_node = None;
    let mut lowest_score = f64::MAX;

    for node in nodes {
        if node.cpu_usage <= constraint.max_cpu
            && node.memory_usage <= constraint.max_memory
            && node.active_sites <= constraint.max_sites
        {
            // Simple scoring: prioritize nodes with lowest overall usage
            let score = node.cpu_usage + node.memory_usage + (node.active_sites as f64 / 100.0);
            if score < lowest_score {
                lowest_score = score;
                best_node = Some(node.node_id);
            }
        }
    }

    best_node.ok_or(PlacementError::NoSuitableNode)
}

pub fn plan_movement(source_node: Uuid, target_node: Uuid, site_id: Uuid) -> MovementPlan {
    MovementPlan {
        source_node,
        target_node,
        site_id,
        steps: vec![
            MovementStep::PrepareTarget,
            MovementStep::SyncData,
            MovementStep::SwitchTraffic,
            MovementStep::CleanupSource,
        ],
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MovementPlan {
    pub source_node: Uuid,
    pub target_node: Uuid,
    pub site_id: Uuid,
    pub steps: Vec<MovementStep>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum MovementStep {
    PrepareTarget,
    SyncData,
    SwitchTraffic,
    CleanupSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plan_placement_success() {
        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();
        let nodes = vec![
            NodeLoad {
                node_id: node1,
                cpu_usage: 0.8,
                memory_usage: 0.9,
                active_sites: 50,
            },
            NodeLoad {
                node_id: node2,
                cpu_usage: 0.2,
                memory_usage: 0.3,
                active_sites: 10,
            },
        ];
        let constraint = PlacementConstraint {
            max_cpu: 0.5,
            max_memory: 0.5,
            max_sites: 20,
        };

        assert_eq!(plan_placement(&nodes, &constraint).unwrap(), node2);
    }

    #[test]
    fn test_plan_placement_no_suitable() {
        let node1 = Uuid::new_v4();
        let nodes = vec![NodeLoad {
            node_id: node1,
            cpu_usage: 0.8,
            memory_usage: 0.9,
            active_sites: 50,
        }];
        let constraint = PlacementConstraint {
            max_cpu: 0.5,
            max_memory: 0.5,
            max_sites: 20,
        };

        assert!(matches!(
            plan_placement(&nodes, &constraint),
            Err(PlacementError::NoSuitableNode)
        ));
    }
}

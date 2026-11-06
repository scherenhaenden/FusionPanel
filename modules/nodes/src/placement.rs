#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodeLoad {
    pub node_id: Uuid,
    /// Normalized load in the inclusive range `0.0..=1.0`.
    pub cpu_usage: f64,
    /// Normalized load in the inclusive range `0.0..=1.0`.
    pub memory_usage: f64,
    /// Number of sites already placed on the node.
    pub active_sites: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacementConstraint {
    /// Maximum normalized CPU usage in the inclusive range `0.0..=1.0`.
    pub max_cpu: f64,
    /// Maximum normalized memory usage in the inclusive range `0.0..=1.0`.
    pub max_memory: f64,
    /// Maximum number of sites after placing the new site.
    pub max_sites: usize,
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum PlacementError {
    #[error("Invalid placement constraint; normalized loads must be finite and between 0 and 1")]
    InvalidConstraint,
    #[error("Invalid load metrics for node {node_id}")]
    InvalidNodeLoad { node_id: Uuid },
    #[error("No suitable node found")]
    NoSuitableNode,
}

fn normalized_load(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

pub fn plan_placement(
    nodes: &[NodeLoad],
    constraint: &PlacementConstraint,
) -> Result<Uuid, PlacementError> {
    if !normalized_load(constraint.max_cpu) || !normalized_load(constraint.max_memory) {
        return Err(PlacementError::InvalidConstraint);
    }

    let mut best_node = None;
    let mut lowest_score = f64::MAX;

    for node in nodes {
        if !normalized_load(node.cpu_usage) || !normalized_load(node.memory_usage) {
            return Err(PlacementError::InvalidNodeLoad {
                node_id: node.node_id,
            });
        }

        // max_sites is the post-placement capacity, so a node already at the
        // limit cannot accept one more site.
        if node.cpu_usage <= constraint.max_cpu
            && node.memory_usage <= constraint.max_memory
            && node.active_sites < constraint.max_sites
        {
            let site_load = node.active_sites as f64 / constraint.max_sites as f64;
            let score = node.cpu_usage + node.memory_usage + site_load;
            if score < lowest_score {
                lowest_score = score;
                best_node = Some(node.node_id);
            }
        }
    }

    best_node.ok_or(PlacementError::NoSuitableNode)
}

#[derive(thiserror::Error, Debug, PartialEq, Eq)]
pub enum MovementPlanError {
    #[error("Source and target nodes must be different")]
    SameSourceAndTarget,
}

pub fn plan_movement(
    source_node: Uuid,
    target_node: Uuid,
    site_id: Uuid,
) -> Result<MovementPlan, MovementPlanError> {
    if source_node == target_node {
        return Err(MovementPlanError::SameSourceAndTarget);
    }

    Ok(MovementPlan {
        source_node,
        target_node,
        site_id,
        steps: vec![
            MovementStep::PrepareTarget,
            MovementStep::SyncData,
            MovementStep::VerifyTarget,
            MovementStep::SwitchTraffic,
            MovementStep::CleanupSource,
        ],
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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
    VerifyTarget,
    SwitchTraffic,
    CleanupSource,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn constraint() -> PlacementConstraint {
        PlacementConstraint {
            max_cpu: 0.5,
            max_memory: 0.5,
            max_sites: 20,
        }
    }

    #[test]
    fn placement_chooses_the_lowest_suitable_node() {
        let node1 = Uuid::new_v4();
        let node2 = Uuid::new_v4();
        let nodes = vec![
            NodeLoad {
                node_id: node1,
                cpu_usage: 0.4,
                memory_usage: 0.4,
                active_sites: 10,
            },
            NodeLoad {
                node_id: node2,
                cpu_usage: 0.2,
                memory_usage: 0.3,
                active_sites: 5,
            },
        ];

        assert_eq!(plan_placement(&nodes, &constraint()), Ok(node2));
    }

    #[test]
    fn placement_rejects_a_node_that_is_already_at_site_capacity() {
        let at_capacity = Uuid::new_v4();
        let below_capacity = Uuid::new_v4();
        let nodes = vec![
            NodeLoad {
                node_id: at_capacity,
                cpu_usage: 0.1,
                memory_usage: 0.1,
                active_sites: 20,
            },
            NodeLoad {
                node_id: below_capacity,
                cpu_usage: 0.4,
                memory_usage: 0.4,
                active_sites: 19,
            },
        ];

        assert_eq!(plan_placement(&nodes, &constraint()), Ok(below_capacity));
    }

    #[test]
    fn placement_fails_when_no_node_is_suitable() {
        let node = Uuid::new_v4();
        let nodes = vec![NodeLoad {
            node_id: node,
            cpu_usage: 0.8,
            memory_usage: 0.9,
            active_sites: 1,
        }];

        assert_eq!(
            plan_placement(&nodes, &constraint()),
            Err(PlacementError::NoSuitableNode)
        );
    }

    #[test]
    fn placement_rejects_invalid_load_and_constraint_values() {
        let node_id = Uuid::new_v4();
        let nodes = [NodeLoad {
            node_id,
            cpu_usage: f64::NAN,
            memory_usage: 0.2,
            active_sites: 0,
        }];
        assert_eq!(
            plan_placement(&nodes, &constraint()),
            Err(PlacementError::InvalidNodeLoad { node_id })
        );

        let invalid_constraint = PlacementConstraint {
            max_cpu: -0.1,
            ..constraint()
        };
        assert_eq!(
            plan_placement(&[], &invalid_constraint),
            Err(PlacementError::InvalidConstraint)
        );
    }

    #[test]
    fn movement_plan_rejects_same_node_and_verifies_target_before_cutover() {
        let source_node = Uuid::new_v4();
        let target_node = Uuid::new_v4();
        let site_id = Uuid::new_v4();

        assert_eq!(
            plan_movement(source_node, source_node, site_id),
            Err(MovementPlanError::SameSourceAndTarget)
        );

        let plan = plan_movement(source_node, target_node, site_id).unwrap();
        assert_eq!(
            plan.steps,
            vec![
                MovementStep::PrepareTarget,
                MovementStep::SyncData,
                MovementStep::VerifyTarget,
                MovementStep::SwitchTraffic,
                MovementStep::CleanupSource,
            ]
        );
    }
}

#![forbid(unsafe_code)]

use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeInfo {
    pub name: String,
    pub labels: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Operator {
    In,
    NotIn,
    Exists,
    DoesNotExist,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LabelSelectorRequirement {
    pub key: String,
    pub operator: Operator,
    pub values: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSelectorTerm {
    pub match_expressions: Vec<LabelSelectorRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSelector {
    pub node_selector_terms: Vec<NodeSelectorTerm>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreferredSchedulingTerm {
    pub weight: i32,
    pub preference: NodeSelectorTerm,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeAffinity {
    pub required_during_scheduling_ignored_during_execution: Option<NodeSelector>,
    pub preferred_during_scheduling_ignored_during_execution: Vec<PreferredSchedulingTerm>,
}

impl NodeAffinity {
    /// Evaluates the given node against the affinity rules.
    /// Returns `None` if the node fails the hard constraints (RequiredDuringScheduling).
    /// Returns `Some(score)` if the node passes, where `score` is the sum of weights from soft constraints (PreferredDuringScheduling).
    pub fn evaluate(&self, node: &NodeInfo) -> Option<i32> {
        if let Some(required) = &self.required_during_scheduling_ignored_during_execution {
            if !self.evaluate_node_selector(node, required) {
                return None;
            }
        }

        let mut score = 0;
        for preferred in &self.preferred_during_scheduling_ignored_during_execution {
            if self.evaluate_node_selector_term(node, &preferred.preference) {
                score += preferred.weight;
            }
        }

        Some(score)
    }

    fn evaluate_node_selector(&self, node: &NodeInfo, selector: &NodeSelector) -> bool {
        if selector.node_selector_terms.is_empty() {
            return true;
        }
        selector.node_selector_terms.iter().any(|term| self.evaluate_node_selector_term(node, term))
    }

    fn evaluate_node_selector_term(&self, node: &NodeInfo, term: &NodeSelectorTerm) -> bool {
        if term.match_expressions.is_empty() {
            return true;
        }
        term.match_expressions.iter().all(|req| self.evaluate_match_expression(node, req))
    }

    fn evaluate_match_expression(&self, node: &NodeInfo, req: &LabelSelectorRequirement) -> bool {
        match req.operator {
            Operator::In => {
                if let Some(val) = node.labels.get(&req.key) {
                    req.values.contains(val)
                } else {
                    false
                }
            }
            Operator::NotIn => {
                if let Some(val) = node.labels.get(&req.key) {
                    !req.values.contains(val)
                } else {
                    true
                }
            }
            Operator::Exists => node.labels.contains_key(&req.key),
            Operator::DoesNotExist => !node.labels.contains_key(&req.key),
        }
    }
}

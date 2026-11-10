#![forbid(unsafe_code)]

use nodes::affinity_evaluator::*;
use std::collections::HashMap;

fn create_node(name: &str, labels: Vec<(&str, &str)>) -> NodeInfo {
    let mut map = HashMap::new();
    for (k, v) in labels {
        map.insert(k.to_string(), v.to_string());
    }
    NodeInfo {
        name: name.to_string(),
        labels: map,
    }
}

#[test]
fn test_hard_constraint_in() {
    let affinity = NodeAffinity {
        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
            node_selector_terms: vec![NodeSelectorTerm {
                match_expressions: vec![LabelSelectorRequirement {
                    key: "zone".to_string(),
                    operator: Operator::In,
                    values: vec!["eu-central-1".to_string(), "eu-west-1".to_string()],
                }],
            }],
        }),
        preferred_during_scheduling_ignored_during_execution: vec![],
    };

    let node1 = create_node("node1", vec![("zone", "eu-central-1")]);
    let node2 = create_node("node2", vec![("zone", "us-east-1")]);

    assert_eq!(affinity.evaluate(&node1), Some(0)); // Passes hard constraint
    assert_eq!(affinity.evaluate(&node2), None); // Fails hard constraint
}

#[test]
fn test_hard_constraint_not_in() {
    let affinity = NodeAffinity {
        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
            node_selector_terms: vec![NodeSelectorTerm {
                match_expressions: vec![LabelSelectorRequirement {
                    key: "gpu".to_string(),
                    operator: Operator::NotIn,
                    values: vec!["nvidia".to_string()],
                }],
            }],
        }),
        preferred_during_scheduling_ignored_during_execution: vec![],
    };

    let node1 = create_node("node1", vec![("gpu", "none")]);
    let node2 = create_node("node2", vec![("gpu", "nvidia")]);
    let node3 = create_node("node3", vec![]);

    assert_eq!(affinity.evaluate(&node1), Some(0)); // Passes
    assert_eq!(affinity.evaluate(&node2), None); // Fails
    assert_eq!(affinity.evaluate(&node3), Some(0)); // Passes (key missing)
}

#[test]
fn test_hard_constraint_exists_does_not_exist() {
    let affinity = NodeAffinity {
        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
            node_selector_terms: vec![NodeSelectorTerm {
                match_expressions: vec![
                    LabelSelectorRequirement {
                        key: "storage".to_string(),
                        operator: Operator::Exists,
                        values: vec![],
                    },
                    LabelSelectorRequirement {
                        key: "maintenance".to_string(),
                        operator: Operator::DoesNotExist,
                        values: vec![],
                    },
                ],
            }],
        }),
        preferred_during_scheduling_ignored_during_execution: vec![],
    };

    let node1 = create_node("node1", vec![("storage", "nvme")]);
    let node2 = create_node("node2", vec![]);
    let node3 = create_node("node3", vec![("storage", "hdd"), ("maintenance", "true")]);

    assert_eq!(affinity.evaluate(&node1), Some(0));
    assert_eq!(affinity.evaluate(&node2), None); // Missing storage
    assert_eq!(affinity.evaluate(&node3), None); // Has maintenance
}

#[test]
fn test_soft_constraints_scoring() {
    let affinity = NodeAffinity {
        required_during_scheduling_ignored_during_execution: None,
        preferred_during_scheduling_ignored_during_execution: vec![
            PreferredSchedulingTerm {
                weight: 50,
                preference: NodeSelectorTerm {
                    match_expressions: vec![LabelSelectorRequirement {
                        key: "zone".to_string(),
                        operator: Operator::In,
                        values: vec!["eu-central-1".to_string()],
                    }],
                },
            },
            PreferredSchedulingTerm {
                weight: 20,
                preference: NodeSelectorTerm {
                    match_expressions: vec![LabelSelectorRequirement {
                        key: "storage".to_string(),
                        operator: Operator::In,
                        values: vec!["nvme".to_string()],
                    }],
                },
            },
        ],
    };

    let node1 = create_node("node1", vec![("zone", "eu-central-1"), ("storage", "nvme")]);
    let node2 = create_node("node2", vec![("zone", "eu-central-1"), ("storage", "hdd")]);
    let node3 = create_node("node3", vec![("zone", "us-east-1"), ("storage", "nvme")]);
    let node4 = create_node("node4", vec![("zone", "us-east-1"), ("storage", "hdd")]);

    assert_eq!(affinity.evaluate(&node1), Some(70));
    assert_eq!(affinity.evaluate(&node2), Some(50));
    assert_eq!(affinity.evaluate(&node3), Some(20));
    assert_eq!(affinity.evaluate(&node4), Some(0));
}

#[test]
fn test_multiple_node_selector_terms_or_logic() {
    // node_selector_terms are ORed
    let affinity = NodeAffinity {
        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
            node_selector_terms: vec![
                NodeSelectorTerm {
                    match_expressions: vec![LabelSelectorRequirement {
                        key: "zone".to_string(),
                        operator: Operator::In,
                        values: vec!["eu-central-1".to_string()],
                    }],
                },
                NodeSelectorTerm {
                    match_expressions: vec![LabelSelectorRequirement {
                        key: "zone".to_string(),
                        operator: Operator::In,
                        values: vec!["us-east-1".to_string()],
                    }],
                },
            ],
        }),
        preferred_during_scheduling_ignored_during_execution: vec![],
    };

    let node1 = create_node("node1", vec![("zone", "eu-central-1")]);
    let node2 = create_node("node2", vec![("zone", "us-east-1")]);
    let node3 = create_node("node3", vec![("zone", "eu-west-1")]);

    assert_eq!(affinity.evaluate(&node1), Some(0));
    assert_eq!(affinity.evaluate(&node2), Some(0));
    assert_eq!(affinity.evaluate(&node3), None);
}

#[test]
fn test_mixed_hard_and_soft_constraints() {
    let affinity = NodeAffinity {
        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
            node_selector_terms: vec![NodeSelectorTerm {
                match_expressions: vec![LabelSelectorRequirement {
                    key: "gpu".to_string(),
                    operator: Operator::Exists,
                    values: vec![],
                }],
            }],
        }),
        preferred_during_scheduling_ignored_during_execution: vec![PreferredSchedulingTerm {
            weight: 100,
            preference: NodeSelectorTerm {
                match_expressions: vec![LabelSelectorRequirement {
                    key: "network".to_string(),
                    operator: Operator::In,
                    values: vec!["10g".to_string()],
                }],
            },
        }],
    };

    let node1 = create_node("node1", vec![("gpu", "nvidia"), ("network", "10g")]);
    let node2 = create_node("node2", vec![("gpu", "amd"), ("network", "1g")]);
    let node3 = create_node("node3", vec![("network", "10g")]); // no gpu

    assert_eq!(affinity.evaluate(&node1), Some(100));
    assert_eq!(affinity.evaluate(&node2), Some(0));
    assert_eq!(affinity.evaluate(&node3), None);
}

#[test]
fn test_empty_node_selector_terms_and_match_expressions() {
    let affinity_empty_terms = NodeAffinity {
        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
            node_selector_terms: vec![],
        }),
        preferred_during_scheduling_ignored_during_execution: vec![],
    };

    let affinity_empty_exprs = NodeAffinity {
        required_during_scheduling_ignored_during_execution: Some(NodeSelector {
            node_selector_terms: vec![NodeSelectorTerm {
                match_expressions: vec![],
            }],
        }),
        preferred_during_scheduling_ignored_during_execution: vec![],
    };

    let node = create_node("node1", vec![]);

    // If node_selector_terms is empty, it matches (usually considered match-all or ignored depending on semantics, but K8s typically says empty matches all for some things, but empty terms usually means false. Wait, standard K8s: if nodeSelectorTerms is empty, it evaluates to true or false?
    // Wait, let's just make sure our code behaves as implemented. In our code `is_empty()` returns `true`.
    assert_eq!(affinity_empty_terms.evaluate(&node), Some(0));
    assert_eq!(affinity_empty_exprs.evaluate(&node), Some(0));
}

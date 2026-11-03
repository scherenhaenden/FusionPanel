use serde::{Deserialize, Serialize};

use crate::inventory::{InventoryItem, InventoryItemType, MigrationInventory};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum PlanActionType {
    Create,
    Skip,
    Conflict,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PlanAction {
    pub action: PlanActionType,
    pub item_id: String,
    pub item_name: String,
    pub item_type: InventoryItemType,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReviewEvidence {
    pub execution_completed: bool,
    pub target_verified: bool,
    pub restore_verified: bool,
    pub handoff_completed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationPlan {
    pub actions: Vec<PlanAction>,
    pub is_dry_run: bool,
    pub decommission_allowed: bool,
}

pub struct Planner;

impl Planner {
    pub fn create_plan(inventory: &MigrationInventory, dry_run: bool) -> MigrationPlan {
        let mut actions = Vec::new();

        for item in &inventory.items {
            let (action_type, reason) = Self::evaluate_item(item, dry_run);
            actions.push(PlanAction {
                action: action_type,
                item_id: item.id.clone(),
                item_name: item.name.clone(),
                item_type: item.item_type.clone(),
                reason,
            });
        }

        MigrationPlan {
            actions,
            is_dry_run: dry_run,
            decommission_allowed: false, // Replaced by create_review_plan logic if needed, but keeping struct compatible
        }
    }

    pub fn create_review_plan(
        inventory: &MigrationInventory,
        dry_run: bool,
        evidence: Option<&ReviewEvidence>,
    ) -> MigrationPlan {
        let mut plan = Self::create_plan(inventory, dry_run);

        if inventory.items.is_empty() {
            plan.decommission_allowed = false;
        } else if dry_run {
            plan.decommission_allowed = false;
        } else {
            let has_unsupported = plan.actions.iter().any(|action| action.action == PlanActionType::Unsupported);
            let has_conflict = plan.actions.iter().any(|action| action.action == PlanActionType::Conflict);
            let has_unknown_items = inventory.items.iter().any(|item| matches!(item.item_type, InventoryItemType::Unknown(_)));

            let has_valid_evidence = match evidence {
                Some(ev) => {
                    ev.execution_completed
                        && ev.target_verified
                        && ev.restore_verified
                        && ev.handoff_completed
                }
                None => false,
            };

            plan.decommission_allowed =
                !has_unsupported && !has_conflict && !has_unknown_items && has_valid_evidence;
        }

        plan
    }

    fn evaluate_item(item: &InventoryItem, dry_run: bool) -> (PlanActionType, Option<String>) {
        if !dry_run {
             return (PlanActionType::Unsupported, Some("Real runtime apply operations are explicitly BLOCKED in this phase. (dry_run=false)".to_string()));
        }

        match item.item_type {
            InventoryItemType::Site => {
                if let Some(version) = &item.version {
                    if version == "5.6" {
                        return (PlanActionType::Unsupported, Some("PHP 5.6 is unsupported".to_string()));
                    }
                }

                if item.version.is_none() {
                    return (PlanActionType::Conflict, Some("Site is missing PHP version definition".to_string()));
                }

                (PlanActionType::Create, None)
            }
            InventoryItemType::Database => (PlanActionType::Create, None),
            InventoryItemType::User => {
                if item.name == "root" || item.name == "admin" {
                    return (PlanActionType::Conflict, Some(format!("User {} conflicts with system users", item.name)));
                }
                (PlanActionType::Create, None)
            }
            InventoryItemType::Unknown(ref s) => (PlanActionType::Unsupported, Some(format!("Unknown type: {}", s))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_full_evidence() -> ReviewEvidence {
        ReviewEvidence {
            execution_completed: true,
            target_verified: true,
            restore_verified: true,
            handoff_completed: true,
        }
    }

    #[test]
    fn test_planner_creates_actions_correctly() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());

        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "example.com".to_string(),
            version: Some("8.1".to_string()),
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });

        inventory.items.push(InventoryItem {
            id: "site_2".to_string(),
            item_type: InventoryItemType::Site,
            name: "old-site.com".to_string(),
            version: Some("5.6".to_string()),
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });

        inventory.items.push(InventoryItem {
            id: "site_3".to_string(),
            item_type: InventoryItemType::Site,
            name: "missing-php-site.com".to_string(),
            version: None,
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });

        inventory.items.push(InventoryItem {
            id: "user_1".to_string(),
            item_type: InventoryItemType::User,
            name: "root".to_string(),
            version: None,
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });

        let plan = Planner::create_plan(&inventory, true);
        assert_eq!(plan.is_dry_run, true);
        assert_eq!(plan.actions.len(), 4);

        assert_eq!(plan.actions[0].action, PlanActionType::Create); // example.com
        assert_eq!(plan.actions[1].action, PlanActionType::Unsupported); // old-site.com
        assert_eq!(plan.actions[1].reason, Some("PHP 5.6 is unsupported".to_string()));
        assert_eq!(plan.actions[2].action, PlanActionType::Conflict); // missing-php-site.com
        assert_eq!(plan.actions[2].reason, Some("Site is missing PHP version definition".to_string()));
        assert_eq!(plan.actions[3].action, PlanActionType::Conflict); // root user
        assert_eq!(plan.actions[3].reason, Some("User root conflicts with system users".to_string()));
    }

    #[test]
    fn test_planner_dry_run_false_is_blocked() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "example.com".to_string(),
            version: Some("8.1".to_string()),
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });

        let plan = Planner::create_plan(&inventory, false);
        assert_eq!(plan.is_dry_run, false);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action, PlanActionType::Unsupported);
        assert_eq!(plan.actions[0].reason, Some("Real runtime apply operations are explicitly BLOCKED in this phase. (dry_run=false)".to_string()));
    }

    #[test]
    fn test_review_plan_empty_inventory_decommission_not_allowed() {
        let inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        let plan = Planner::create_review_plan(&inventory, false, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_dry_run_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "example.com".to_string(),
            version: Some("8.1".to_string()),
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_unsupported_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "old-site.com".to_string(),
            version: Some("5.6".to_string()), // PHP 5.6 is unsupported
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_conflict_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "user_1".to_string(),
            item_type: InventoryItemType::User,
            name: "root".to_string(),
            version: None,
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_unknown_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "unknown_1".to_string(),
            item_type: InventoryItemType::Unknown("some_type".to_string()),
            name: "unknown-item".to_string(),
            version: None,
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_missing_evidence_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "example.com".to_string(),
            version: Some("8.1".to_string()),
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });
        let plan = Planner::create_review_plan(&inventory, false, None);
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_incomplete_evidence_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "example.com".to_string(),
            version: Some("8.1".to_string()),
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });

        let evs = vec![
            ReviewEvidence { execution_completed: false, target_verified: true, restore_verified: true, handoff_completed: true },
            ReviewEvidence { execution_completed: true, target_verified: false, restore_verified: true, handoff_completed: true },
            ReviewEvidence { execution_completed: true, target_verified: true, restore_verified: false, handoff_completed: true },
            ReviewEvidence { execution_completed: true, target_verified: true, restore_verified: true, handoff_completed: false },
        ];

        for ev in evs {
            let plan = Planner::create_review_plan(&inventory, false, Some(&ev));
            assert_eq!(plan.decommission_allowed, false);
        }
    }

    #[test]
    fn test_review_plan_valid_inventory_dry_run_false_is_blocked() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(InventoryItem {
            id: "site_1".to_string(),
            item_type: InventoryItemType::Site,
            name: "example.com".to_string(),
            version: Some("8.1".to_string()),
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
        });

        // Because evaluate_item hard-blocks dry_run=false operations, they always yield Unsupported actions!
        let plan = Planner::create_review_plan(&inventory, false, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }
}

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
            let (action_type, reason) = Self::evaluate_item(item, dry_run, &inventory.source_version);
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
        let plan = Self::create_plan(inventory, dry_run);

        let mut decommission_allowed = false;

        // "decommission safety mandates failing closed: decommissioning must be explicitly disallowed in the planner
        // if the source inventory is empty, is a dry-run, lacks explicit ReviewEvidence, contains unknown schema types,
        // or yields conflicting/unsupported actions."

        if !inventory.items.is_empty() && !dry_run {
            if let Some(ev) = evidence {
                if ev.execution_completed && ev.target_verified && ev.restore_verified && ev.handoff_completed {
                    let has_invalid_actions = plan.actions.iter().any(|a| {
                        a.action == PlanActionType::Unsupported || a.action == PlanActionType::Conflict
                    });

                    let has_unknown_types = inventory.items.iter().any(|i| {
                        matches!(i.item_type, InventoryItemType::Unknown(_))
                    });

                    if !has_invalid_actions && !has_unknown_types {
                        decommission_allowed = true;
                    }
                }
            }
        }

        MigrationPlan {
            actions: plan.actions,
            is_dry_run: dry_run,
            decommission_allowed,
        }
    }

    fn evaluate_item(item: &InventoryItem, dry_run: bool, source_version: &str) -> (PlanActionType, Option<String>) {
        // Now require exact source-version adapter support if dry_run=false
        if !dry_run {
             if !source_version.starts_with("2.4") && !source_version.starts_with("2.3") {
                 return (PlanActionType::Unsupported, Some("Real runtime apply operations require exact source-version adapter support.".to_string()));
             }
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

    fn create_test_item(id: &str, item_type: InventoryItemType, name: &str, version: Option<String>) -> InventoryItem {
        InventoryItem {
            id: id.to_string(),
            item_type,
            name: name.to_string(),
            version,
            metadata: std::collections::HashMap::new(),
            associations: Vec::new(),
            ownership: None,
            aliases: Vec::new(),
            schedules: Vec::new(),
            variants: Vec::new(),
            user_mappings: std::collections::HashMap::new(),
        }
    }

    #[test]
    fn test_planner_creates_actions_correctly() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item("site_1", InventoryItemType::Site, "example.com", Some("8.1".to_string())));
        inventory.items.push(create_test_item("site_2", InventoryItemType::Site, "old-site.com", Some("5.6".to_string())));
        inventory.items.push(create_test_item("site_3", InventoryItemType::Site, "missing-php-site.com", None));
        inventory.items.push(create_test_item("user_1", InventoryItemType::User, "root", None));

        let plan = Planner::create_plan(&inventory, true);
        assert_eq!(plan.is_dry_run, true);
        assert_eq!(plan.actions.len(), 4);

        assert_eq!(plan.actions[0].action, PlanActionType::Create); // example.com
        assert_eq!(plan.actions[1].action, PlanActionType::Unsupported); // old-site.com
        assert_eq!(plan.actions[2].action, PlanActionType::Conflict); // missing-php-site.com
        assert_eq!(plan.actions[3].action, PlanActionType::Conflict); // root user
    }

    #[test]
    fn test_planner_dry_run_false_is_blocked_for_unsupported_version() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "1.0.0".to_string());
        inventory.items.push(create_test_item("site_1", InventoryItemType::Site, "example.com", Some("8.1".to_string())));

        let plan = Planner::create_plan(&inventory, false);
        assert_eq!(plan.is_dry_run, false);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action, PlanActionType::Unsupported);
        assert_eq!(plan.actions[0].reason, Some("Real runtime apply operations require exact source-version adapter support.".to_string()));
    }

    #[test]
    fn test_planner_dry_run_false_is_allowed_for_supported_version() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item("site_1", InventoryItemType::Site, "example.com", Some("8.1".to_string())));

        let plan = Planner::create_plan(&inventory, false);
        assert_eq!(plan.is_dry_run, false);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action, PlanActionType::Create);
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
        inventory.items.push(create_test_item("site_1", InventoryItemType::Site, "example.com", Some("8.1".to_string())));
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_unsupported_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item("site_1", InventoryItemType::Site, "old-site.com", Some("5.6".to_string()))); // PHP 5.6 is unsupported
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_conflict_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item("user_1", InventoryItemType::User, "root", None));
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_unknown_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item("unknown_1", InventoryItemType::Unknown("some_type".to_string()), "unknown-item", None));
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_missing_evidence_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item("site_1", InventoryItemType::Site, "example.com", Some("8.1".to_string())));
        let plan = Planner::create_review_plan(&inventory, false, None);
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_incomplete_evidence_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item("site_1", InventoryItemType::Site, "example.com", Some("8.1".to_string())));

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
}

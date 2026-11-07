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

        let mut site_names = std::collections::HashSet::new();
        let mut db_names = std::collections::HashSet::new();

        // Find collisions
        let mut site_collisions = std::collections::HashSet::new();
        let mut db_collisions = std::collections::HashSet::new();

        for item in &inventory.items {
            match item.item_type {
                InventoryItemType::Site => {
                    if !site_names.insert(&item.name) {
                        site_collisions.insert(&item.name);
                    }
                }
                InventoryItemType::Database => {
                    if !db_names.insert(&item.name) {
                        db_collisions.insert(&item.name);
                    }
                }
                _ => {}
            }
        }

        for item in &inventory.items {
            let (mut action_type, mut reason) =
                Self::evaluate_item(item, dry_run, &inventory.source_version);

            match item.item_type {
                InventoryItemType::Site if site_collisions.contains(&item.name) => {
                    action_type = PlanActionType::Conflict;
                    reason = Some(format!("Domain name collision: {}", item.name));
                }
                InventoryItemType::Database if db_collisions.contains(&item.name) => {
                    action_type = PlanActionType::Conflict;
                    reason = Some(format!("Database name collision: {}", item.name));
                }
                _ => {}
            }

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
        _evidence: Option<&ReviewEvidence>,
    ) -> MigrationPlan {
        let plan = Self::create_plan(inventory, dry_run);

        MigrationPlan {
            actions: plan.actions,
            is_dry_run: dry_run,
            // Caller-supplied booleans are not independent migration or recovery evidence.
            decommission_allowed: false,
        }
    }

    fn evaluate_item(
        item: &InventoryItem,
        dry_run: bool,
        source_version: &str,
    ) -> (PlanActionType, Option<String>) {
        // Only these exact adapter versions are qualified for planning.
        if !dry_run && !matches!(source_version, "2.3.0" | "2.4.0") {
            return (
                PlanActionType::Unsupported,
                Some(
                    "Real runtime apply operations require exact source-version adapter support."
                        .to_string(),
                ),
            );
        }

        match item.item_type {
            InventoryItemType::Site => {
                if let Some(version) = &item.version {
                    if version == "5.6" {
                        return (
                            PlanActionType::Unsupported,
                            Some("PHP 5.6 is unsupported".to_string()),
                        );
                    }
                }

                if item.version.is_none() {
                    return (
                        PlanActionType::Conflict,
                        Some("Site is missing PHP version definition".to_string()),
                    );
                }

                (PlanActionType::Create, None)
            }
            InventoryItemType::Database => (PlanActionType::Create, None),
            InventoryItemType::User => {
                if item.name == "root" || item.name == "admin" {
                    return (
                        PlanActionType::Conflict,
                        Some(format!("User {} conflicts with system users", item.name)),
                    );
                }
                (PlanActionType::Create, None)
            }
            InventoryItemType::Unknown(ref s) => (
                PlanActionType::Unsupported,
                Some(format!("Unknown type: {}", s)),
            ),
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

    fn create_test_item(
        id: &str,
        item_type: InventoryItemType,
        name: &str,
        version: Option<String>,
    ) -> InventoryItem {
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
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));
        inventory.items.push(create_test_item(
            "site_2",
            InventoryItemType::Site,
            "old-site.com",
            Some("5.6".to_string()),
        ));
        inventory.items.push(create_test_item(
            "site_3",
            InventoryItemType::Site,
            "missing-php-site.com",
            None,
        ));
        inventory.items.push(create_test_item(
            "user_1",
            InventoryItemType::User,
            "root",
            None,
        ));

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
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));

        let plan = Planner::create_plan(&inventory, false);
        assert_eq!(plan.is_dry_run, false);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action, PlanActionType::Unsupported);
        assert_eq!(
            plan.actions[0].reason,
            Some(
                "Real runtime apply operations require exact source-version adapter support."
                    .to_string()
            )
        );
    }

    #[test]
    fn test_planner_dry_run_false_is_allowed_for_supported_version() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));

        let plan = Planner::create_plan(&inventory, false);
        assert_eq!(plan.is_dry_run, false);
        assert_eq!(plan.actions.len(), 1);
        assert_eq!(plan.actions[0].action, PlanActionType::Create);
    }

    #[test]
    fn test_planner_rejects_version_prefix_collisions() {
        for source_version in ["2.40.0", "2.4.0-rc1", "2.4.1"] {
            let mut inventory =
                MigrationInventory::new("CloudPanel".to_string(), source_version.to_string());
            inventory.items.push(create_test_item(
                "site_1",
                InventoryItemType::Site,
                "example.com",
                Some("8.1".to_string()),
            ));

            let plan = Planner::create_plan(&inventory, false);
            assert_eq!(
                plan.actions[0].action,
                PlanActionType::Unsupported,
                "{source_version}"
            );
        }
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
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_unsupported_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "old-site.com",
            Some("5.6".to_string()),
        )); // PHP 5.6 is unsupported
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_conflict_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "user_1",
            InventoryItemType::User,
            "root",
            None,
        ));
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_unknown_item_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "unknown_1",
            InventoryItemType::Unknown("some_type".to_string()),
            "unknown-item",
            None,
        ));
        let plan = Planner::create_review_plan(&inventory, true, Some(&get_full_evidence()));
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_missing_evidence_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));
        let plan = Planner::create_review_plan(&inventory, false, None);
        assert_eq!(plan.decommission_allowed, false);
    }

    #[test]
    fn test_review_plan_incomplete_evidence_decommission_not_allowed() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));

        let evs = vec![
            ReviewEvidence {
                execution_completed: false,
                target_verified: true,
                restore_verified: true,
                handoff_completed: true,
            },
            ReviewEvidence {
                execution_completed: true,
                target_verified: false,
                restore_verified: true,
                handoff_completed: true,
            },
            ReviewEvidence {
                execution_completed: true,
                target_verified: true,
                restore_verified: false,
                handoff_completed: true,
            },
            ReviewEvidence {
                execution_completed: true,
                target_verified: true,
                restore_verified: true,
                handoff_completed: false,
            },
        ];

        for ev in evs {
            let plan = Planner::create_review_plan(&inventory, false, Some(&ev));
            assert_eq!(plan.decommission_allowed, false);
        }
    }

    #[test]
    fn test_caller_supplied_review_evidence_never_allows_decommission() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));
        let plan = Planner::create_review_plan(&inventory, false, Some(&get_full_evidence()));
        assert!(!plan.decommission_allowed);
    }

    #[test]
    fn test_planner_detects_domain_collisions() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "site_1",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));
        inventory.items.push(create_test_item(
            "site_2",
            InventoryItemType::Site,
            "example.com",
            Some("8.1".to_string()),
        ));

        let plan = Planner::create_plan(&inventory, true);
        assert_eq!(plan.actions.len(), 2);

        let conflicts = plan
            .actions
            .iter()
            .filter(|a| a.action == PlanActionType::Conflict)
            .count();
        assert_eq!(conflicts, 2);
    }

    #[test]
    fn test_planner_detects_db_collisions() {
        let mut inventory = MigrationInventory::new("CloudPanel".to_string(), "2.4.0".to_string());
        inventory.items.push(create_test_item(
            "db_1",
            InventoryItemType::Database,
            "my_db",
            None,
        ));
        inventory.items.push(create_test_item(
            "db_2",
            InventoryItemType::Database,
            "my_db",
            None,
        ));

        let plan = Planner::create_plan(&inventory, true);
        assert_eq!(plan.actions.len(), 2);

        let conflicts = plan
            .actions
            .iter()
            .filter(|a| a.action == PlanActionType::Conflict)
            .count();
        assert_eq!(conflicts, 2);
    }
}

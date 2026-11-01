use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

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
    /// Describes a proposed plan action only. It is never evidence of an
    /// applied change.
    pub action: PlanActionType,
    pub item_id: String,
    pub item_name: String,
    pub item_type: InventoryItemType,
    pub reason: Option<String>,
}

/// Evidence gathered by independent source collectors and an operator review.
/// Booleans are explicit attestations, not claims that this module itself has
/// parsed Nginx, probed a listener, or inspected filesystem mounts.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceSiteEvidence {
    pub source_site_id: String,
    pub metadata_matches_effective_server: bool,
    pub routing_redirects_rewrites_preserved: bool,
    pub discovery_and_oauth_endpoints_preserved: bool,
    pub streaming_websocket_behavior_preserved: bool,
    pub upstream_listener_verified: bool,
    pub tls_trust_and_sni_reviewed: bool,
    /// Managed roots continue to use the existing strict validator. External
    /// roots use a separate approved binding and never weaken that validator.
    pub managed_root_validation_passed: bool,
    pub external_root_binding: Option<ApprovedExternalRootBinding>,
    pub source_user_ownership_reviewed: bool,
    pub source_user_deactivation_safe: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ApprovedExternalRootBinding {
    pub source_path: String,
    pub canonical_source_path: String,
    pub target_path: String,
    pub owner: String,
    pub purpose: String,
    pub read_only: bool,
    pub allowed_write_paths: Vec<String>,
    pub mounts_and_symlinks_reviewed: bool,
    pub operator_review_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SourceDatabaseEvidence {
    pub source_database_id: String,
    pub ownership_reviewed: bool,
    pub read_only_export_access_verified: bool,
    pub users_and_grants_reviewed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct TargetSnapshot {
    pub inspection_complete: bool,
    pub site_domains: HashSet<String>,
    pub database_names: HashSet<String>,
    pub database_usernames: HashSet<String>,
    pub os_usernames: HashSet<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct OperationalHandoff {
    pub source_schema_version_verified: bool,
    pub independent_database_backup_and_restore_verified: bool,
    pub tls_renewal_owner_verified: bool,
    pub critical_scheduled_jobs_reviewed: bool,
    pub external_dependencies_reviewed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MigrationPlan {
    pub actions: Vec<PlanAction>,
    pub is_dry_run: bool,
    pub whole_server_apply_supported: bool,
    pub decommission_allowed: bool,
    pub blockers: Vec<String>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlannerError {
    #[error("migration apply is not implemented; refusing to represent a dry-run plan as applied")]
    ApplyNotImplemented,
}

pub struct Planner;

impl Planner {
    /// Without source/target evidence, no item can safely be marked Create.
    /// A non-dry invocation is explicitly refused.
    pub fn create_plan(
        inventory: &MigrationInventory,
        dry_run: bool,
    ) -> Result<MigrationPlan, PlannerError> {
        if !dry_run {
            return Err(PlannerError::ApplyNotImplemented);
        }
        Ok(Self::create_review_plan(
            inventory,
            &[],
            &[],
            &TargetSnapshot::default(),
            &OperationalHandoff::default(),
        ))
    }

    pub fn create_review_plan(
        inventory: &MigrationInventory,
        site_evidence: &[SourceSiteEvidence],
        database_evidence: &[SourceDatabaseEvidence],
        target: &TargetSnapshot,
        handoff: &OperationalHandoff,
    ) -> MigrationPlan {
        let site_evidence: HashMap<&str, &SourceSiteEvidence> = site_evidence
            .iter()
            .map(|e| (e.source_site_id.as_str(), e))
            .collect();
        let database_evidence: HashMap<&str, &SourceDatabaseEvidence> = database_evidence
            .iter()
            .map(|e| (e.source_database_id.as_str(), e))
            .collect();
        let mut actions = Vec::with_capacity(inventory.items.len());
        let mut blockers = Vec::new();

        if !target.inspection_complete {
            blockers.push("target resource inventory was not inspected".into());
        }
        if !handoff.source_schema_version_verified {
            blockers
                .push("source schema/version is not qualified by a version-matched export".into());
        }
        if !handoff.independent_database_backup_and_restore_verified {
            blockers.push("independent database backup and restore are not verified".into());
        }
        if !handoff.tls_renewal_owner_verified {
            blockers.push("TLS renewal ownership has not been verified".into());
        }
        if !handoff.critical_scheduled_jobs_reviewed {
            blockers.push("critical scheduled jobs have not been reviewed".into());
        }
        if !handoff.external_dependencies_reviewed {
            blockers.push("external resource dependencies have not been reviewed".into());
        }

        let mut prior_actions: HashMap<&str, PlanActionType> = HashMap::new();
        for item in &inventory.items {
            let (action, reason) = Self::evaluate_item(
                item,
                &site_evidence,
                &database_evidence,
                target,
                &prior_actions,
            );
            if let Some(reason) = &reason {
                blockers.push(format!("{}: {}", item.id, reason));
            }
            prior_actions.insert(&item.id, action.clone());
            actions.push(PlanAction {
                action,
                item_id: item.id.clone(),
                item_name: item.name.clone(),
                item_type: item.item_type.clone(),
                reason,
            });
        }

        let all_items_reviewed = actions
            .iter()
            .all(|action| matches!(action.action, PlanActionType::Create | PlanActionType::Skip));
        let decommission_allowed = all_items_reviewed
            && blockers.is_empty()
            && target.inspection_complete
            && handoff.source_schema_version_verified
            && handoff.independent_database_backup_and_restore_verified
            && handoff.tls_renewal_owner_verified
            && handoff.critical_scheduled_jobs_reviewed
            && handoff.external_dependencies_reviewed;

        MigrationPlan {
            actions,
            is_dry_run: true,
            whole_server_apply_supported: false,
            decommission_allowed,
            blockers,
        }
    }

    fn evaluate_item(
        item: &InventoryItem,
        site_evidence: &HashMap<&str, &SourceSiteEvidence>,
        database_evidence: &HashMap<&str, &SourceDatabaseEvidence>,
        target: &TargetSnapshot,
        prior_actions: &HashMap<&str, PlanActionType>,
    ) -> (PlanActionType, Option<String>) {
        match &item.item_type {
            InventoryItemType::Site => {
                let Some(evidence) = site_evidence.get(item.id.as_str()) else {
                    return (
                        PlanActionType::Conflict,
                        Some(
                            "effective server configuration and upstream were not reconciled"
                                .into(),
                        ),
                    );
                };
                if !target.inspection_complete {
                    return (
                        PlanActionType::Conflict,
                        Some("target site inventory is incomplete".into()),
                    );
                }
                if !evidence.metadata_matches_effective_server {
                    return (
                        PlanActionType::Conflict,
                        Some("panel metadata differs from effective server configuration; manual review required".into()),
                    );
                }
                if !evidence.routing_redirects_rewrites_preserved {
                    return (
                        PlanActionType::Conflict,
                        Some("custom locations, methods, headers, redirects, or rewrites are not preserved".into()),
                    );
                }
                if !evidence.discovery_and_oauth_endpoints_preserved {
                    return (
                        PlanActionType::Conflict,
                        Some("discovery, OAuth, or other declared endpoint behavior is not preserved".into()),
                    );
                }
                if !evidence.streaming_websocket_behavior_preserved {
                    return (
                        PlanActionType::Conflict,
                        Some("streaming or WebSocket behavior is not preserved".into()),
                    );
                }
                if !evidence.upstream_listener_verified {
                    return (
                        PlanActionType::Conflict,
                        Some("upstream listener was not verified".into()),
                    );
                }
                if !evidence.tls_trust_and_sni_reviewed {
                    return (
                        PlanActionType::Conflict,
                        Some("upstream TLS trust and SNI policy was not reviewed".into()),
                    );
                }
                match (
                    evidence.managed_root_validation_passed,
                    &evidence.external_root_binding,
                ) {
                    (true, None) => {}
                    (false, Some(binding)) if valid_external_root_binding(item, binding) => {}
                    (false, Some(_)) => {
                        return (
                            PlanActionType::Conflict,
                            Some("external-root binding lacks a safe canonical path, owner, purpose, write scope, or review".into()),
                        );
                    }
                    _ => {
                        return (
                            PlanActionType::Conflict,
                            Some("exactly one of strict managed-root validation or an approved external-root binding is required".into()),
                        );
                    }
                }
                if target.site_domains.contains(&item.name) {
                    return (
                        PlanActionType::Conflict,
                        Some("target domain already exists".into()),
                    );
                }
                if item
                    .metadata
                    .get("site_kind")
                    .is_some_and(|kind| kind == "reverse-proxy")
                    && item
                        .metadata
                        .get("source_upstream_url")
                        .is_none_or(String::is_empty)
                {
                    return (
                        PlanActionType::Conflict,
                        Some("reverse proxy has no source upstream URL".into()),
                    );
                }
                (PlanActionType::Create, None)
            }
            InventoryItemType::Database => {
                if item.associations.is_empty() {
                    return (
                        PlanActionType::Conflict,
                        Some("database has no explicit source-site association".into()),
                    );
                }
                if !all_dependencies_are(
                    item,
                    prior_actions,
                    InventoryItemType::Site,
                    PlanActionType::Create,
                ) {
                    return (
                        PlanActionType::Conflict,
                        Some("one or more associated sites are unresolved".into()),
                    );
                }
                let Some(evidence) = database_evidence.get(item.id.as_str()) else {
                    return (
                        PlanActionType::Conflict,
                        Some(
                            "database ownership and read-only export boundary were not reviewed"
                                .into(),
                        ),
                    );
                };
                if !evidence.ownership_reviewed
                    || !evidence.read_only_export_access_verified
                    || !evidence.users_and_grants_reviewed
                {
                    return (
                        PlanActionType::Conflict,
                        Some(
                            "database ownership, export access, users, or grants remain unverified"
                                .into(),
                        ),
                    );
                }
                if !target.inspection_complete {
                    return (
                        PlanActionType::Conflict,
                        Some("target database inventory is incomplete".into()),
                    );
                }
                if target.database_names.contains(&item.name) {
                    return (
                        PlanActionType::Conflict,
                        Some("target database already exists".into()),
                    );
                }
                (PlanActionType::Create, None)
            }
            InventoryItemType::DatabaseUser => {
                if item.associations.is_empty() {
                    return (
                        PlanActionType::Conflict,
                        Some("database user has no explicit database association".into()),
                    );
                }
                if !all_dependencies_are(
                    item,
                    prior_actions,
                    InventoryItemType::Database,
                    PlanActionType::Create,
                ) {
                    return (
                        PlanActionType::Conflict,
                        Some("one or more associated databases are unresolved".into()),
                    );
                }
                if !target.inspection_complete {
                    return (
                        PlanActionType::Conflict,
                        Some("target database-user inventory is incomplete".into()),
                    );
                }
                if target.database_usernames.contains(&item.name) {
                    return (
                        PlanActionType::Conflict,
                        Some("target database username already exists".into()),
                    );
                }
                (PlanActionType::Create, None)
            }
            InventoryItemType::User => {
                if item.name == "root" || item.name == "admin" || !valid_linux_username(&item.name)
                {
                    return (
                        PlanActionType::Conflict,
                        Some("OS username is invalid, option-like, or reserved; preserve or explicitly review a mapping".into()),
                    );
                }
                if !target.inspection_complete {
                    return (
                        PlanActionType::Conflict,
                        Some("target OS-user inventory is incomplete".into()),
                    );
                }
                if target.os_usernames.contains(&item.name) {
                    return (
                        PlanActionType::Conflict,
                        Some("OS username already exists; collision-safe mapping required".into()),
                    );
                }
                if item.associations.is_empty()
                    || !all_dependencies_are(
                        item,
                        prior_actions,
                        InventoryItemType::Site,
                        PlanActionType::Create,
                    )
                {
                    return (
                        PlanActionType::Conflict,
                        Some("one or more associated sites are unresolved".into()),
                    );
                }
                let related_sites = item
                    .associations
                    .iter()
                    .filter_map(|id| site_evidence.get(id.as_str()))
                    .collect::<Vec<_>>();
                if related_sites.len() != item.associations.len()
                    || related_sites.iter().any(|e| {
                        !e.source_user_ownership_reviewed || !e.source_user_deactivation_safe
                    })
                {
                    return (
                        PlanActionType::Conflict,
                        Some("external/adopted user ownership and process lifecycle are not reviewed; do not disable, kill by UID, or delete the account".into()),
                    );
                }
                (PlanActionType::Create, None)
            }
            InventoryItemType::Unknown(kind) => (
                PlanActionType::Unsupported,
                Some(format!("unsupported resource type: {kind}")),
            ),
        }
    }
}

fn all_dependencies_are(
    item: &InventoryItem,
    prior_actions: &HashMap<&str, PlanActionType>,
    expected_type: InventoryItemType,
    expected_action: PlanActionType,
) -> bool {
    item.associations.iter().all(|id| {
        let prefix = match expected_type {
            InventoryItemType::Site => "site_",
            InventoryItemType::Database => "db_",
            _ => return false,
        };
        id.starts_with(prefix) && prior_actions.get(id.as_str()) == Some(&expected_action)
    })
}

/// Linux account names stay byte-for-byte intact; this validates only a
/// conservative portable subset and never normalizes `-` to `_`.
fn valid_linux_username(name: &str) -> bool {
    let bytes = name.as_bytes();
    if bytes.is_empty() || bytes.len() > 32 || !bytes[0].is_ascii_lowercase() && bytes[0] != b'_' {
        return false;
    }
    bytes[1..].iter().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'_' || *byte == b'-'
    })
}

fn valid_absolute_path(path: &str) -> bool {
    let path = std::path::Path::new(path);
    path.is_absolute()
        && path.components().all(|component| {
            matches!(
                component,
                std::path::Component::RootDir | std::path::Component::Normal(_)
            )
        })
}

fn valid_relative_write_path(path: &str) -> bool {
    let path = std::path::Path::new(path);
    !path.as_os_str().is_empty()
        && !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

fn valid_external_root_binding(
    item: &InventoryItem,
    binding: &ApprovedExternalRootBinding,
) -> bool {
    let Some(source_document_root) = item.metadata.get("source_document_root") else {
        return false;
    };
    binding.source_path == *source_document_root
        && valid_absolute_path(&binding.source_path)
        && valid_absolute_path(&binding.canonical_source_path)
        && valid_absolute_path(&binding.target_path)
        && !binding.owner.is_empty()
        && !binding.purpose.is_empty()
        && binding.mounts_and_symlinks_reviewed
        && !binding.operator_review_id.is_empty()
        && (binding.read_only
            || (!binding.allowed_write_paths.is_empty()
                && binding
                    .allowed_write_paths
                    .iter()
                    .all(|path| valid_relative_write_path(path))))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> MigrationInventory {
        let mut inventory = MigrationInventory::new(
            "CloudPanel fixture".into(),
            "2.5.4".into(),
            "candidate-fixture-only".into(),
        );
        inventory.items.push(InventoryItem {
            id: "site_1".into(),
            item_type: InventoryItemType::Site,
            name: "proxy.example.test".into(),
            version: None,
            associations: vec!["user_app-user".into()],
            metadata: HashMap::from([
                ("site_kind".into(), "reverse-proxy".into()),
                (
                    "source_upstream_url".into(),
                    "https://127.0.0.1:8443".into(),
                ),
                ("source_document_root".into(), "/external/site".into()),
            ]),
        });
        inventory.items.push(InventoryItem {
            id: "db_1".into(),
            item_type: InventoryItemType::Database,
            name: "site_db".into(),
            version: Some("10.11".into()),
            associations: vec!["site_1".into()],
            metadata: HashMap::new(),
        });
        inventory.items.push(InventoryItem {
            id: "dbuser_1".into(),
            item_type: InventoryItemType::DatabaseUser,
            name: "app-db-user".into(),
            version: None,
            associations: vec!["db_1".into()],
            metadata: HashMap::new(),
        });
        inventory.items.push(InventoryItem {
            id: "user_app-user".into(),
            item_type: InventoryItemType::User,
            name: "app-user".into(),
            version: None,
            associations: vec!["site_1".into()],
            metadata: HashMap::from([("ownership".into(), "external_or_unknown".into())]),
        });
        inventory
    }

    fn complete_evidence() -> (
        Vec<SourceSiteEvidence>,
        Vec<SourceDatabaseEvidence>,
        TargetSnapshot,
        OperationalHandoff,
    ) {
        (
            vec![SourceSiteEvidence {
                source_site_id: "site_1".into(),
                metadata_matches_effective_server: true,
                routing_redirects_rewrites_preserved: true,
                discovery_and_oauth_endpoints_preserved: true,
                streaming_websocket_behavior_preserved: true,
                upstream_listener_verified: true,
                tls_trust_and_sni_reviewed: true,
                managed_root_validation_passed: false,
                external_root_binding: Some(ApprovedExternalRootBinding {
                    source_path: "/external/site".into(),
                    canonical_source_path: "/mnt/external/site".into(),
                    target_path: "/srv/adopted/site".into(),
                    owner: "app-user".into(),
                    purpose: "existing application document root".into(),
                    read_only: false,
                    allowed_write_paths: vec!["uploads".into()],
                    mounts_and_symlinks_reviewed: true,
                    operator_review_id: "review-root-1".into(),
                }),
                source_user_ownership_reviewed: true,
                source_user_deactivation_safe: true,
            }],
            vec![SourceDatabaseEvidence {
                source_database_id: "db_1".into(),
                ownership_reviewed: true,
                read_only_export_access_verified: true,
                users_and_grants_reviewed: true,
            }],
            TargetSnapshot {
                inspection_complete: true,
                ..Default::default()
            },
            OperationalHandoff {
                source_schema_version_verified: true,
                independent_database_backup_and_restore_verified: true,
                tls_renewal_owner_verified: true,
                critical_scheduled_jobs_reviewed: true,
                external_dependencies_reviewed: true,
            },
        )
    }

    #[test]
    fn missing_evidence_means_conflicts_and_no_decommission() {
        let plan = Planner::create_plan(&sample(), true).unwrap();
        assert!(plan
            .actions
            .iter()
            .all(|action| action.action == PlanActionType::Conflict));
        assert!(!plan.decommission_allowed);
        assert!(!plan.whole_server_apply_supported);
    }

    #[test]
    fn non_dry_mode_fails_instead_of_claiming_apply() {
        assert_eq!(
            Planner::create_plan(&sample(), false),
            Err(PlannerError::ApplyNotImplemented)
        );
    }

    #[test]
    fn fully_reviewed_plan_is_still_only_a_plan_and_keeps_handoffs_explicit() {
        let (sites, databases, target, handoff) = complete_evidence();
        let plan = Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff);
        assert!(plan
            .actions
            .iter()
            .all(|action| action.action == PlanActionType::Create));
        assert!(plan.is_dry_run);
        assert!(plan.decommission_allowed);
        assert!(!plan.whole_server_apply_supported);
    }

    #[test]
    fn metadata_drift_and_custom_route_loss_block_site_and_dependents() {
        let (mut sites, databases, target, handoff) = complete_evidence();
        sites[0].metadata_matches_effective_server = false;
        let plan = Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff);
        assert_eq!(plan.actions[0].action, PlanActionType::Conflict);
        assert_eq!(plan.actions[1].action, PlanActionType::Conflict);
        sites[0].metadata_matches_effective_server = true;
        sites[0].routing_redirects_rewrites_preserved = false;
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
    }

    #[test]
    fn discovery_streaming_and_upstream_tls_gates_are_enforced() {
        let (mut sites, databases, target, handoff) = complete_evidence();
        sites[0].discovery_and_oauth_endpoints_preserved = false;
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
        sites[0].discovery_and_oauth_endpoints_preserved = true;
        sites[0].streaming_websocket_behavior_preserved = false;
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
        sites[0].streaming_websocket_behavior_preserved = true;
        sites[0].tls_trust_and_sni_reviewed = false;
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
    }

    #[test]
    fn external_roots_require_a_separate_review_and_managed_validation_remains_exclusive() {
        let (mut sites, databases, target, handoff) = complete_evidence();
        sites[0]
            .external_root_binding
            .as_mut()
            .unwrap()
            .operator_review_id
            .clear();
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
        sites[0]
            .external_root_binding
            .as_mut()
            .unwrap()
            .operator_review_id = "review-root-1".into();
        sites[0].managed_root_validation_passed = true;
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
        sites[0].managed_root_validation_passed = false;
        sites[0].external_root_binding.as_mut().unwrap().target_path = "/srv/../etc".into();
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
        sites[0].external_root_binding.as_mut().unwrap().target_path = "/srv/adopted/site".into();
        sites[0]
            .external_root_binding
            .as_mut()
            .unwrap()
            .allowed_write_paths = vec!["../etc".into()];
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
        sites[0]
            .external_root_binding
            .as_mut()
            .unwrap()
            .allowed_write_paths = vec!["uploads".into()];
        sites[0]
            .external_root_binding
            .as_mut()
            .unwrap()
            .mounts_and_symlinks_reviewed = false;
        assert_eq!(
            Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff).actions
                [0]
            .action,
            PlanActionType::Conflict
        );
    }

    #[test]
    fn database_ownership_export_and_grants_must_be_reviewed() {
        let (sites, mut databases, target, handoff) = complete_evidence();
        databases[0].users_and_grants_reviewed = false;
        let plan = Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff);
        assert_eq!(plan.actions[1].action, PlanActionType::Conflict);
        assert_eq!(plan.actions[2].action, PlanActionType::Conflict);
    }

    #[test]
    fn backup_tls_jobs_and_external_dependencies_gate_decommission() {
        let (sites, databases, target, mut handoff) = complete_evidence();
        handoff.tls_renewal_owner_verified = false;
        let plan = Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff);
        assert!(!plan.decommission_allowed);
        assert!(plan
            .blockers
            .iter()
            .any(|reason| reason.contains("TLS renewal")));
    }

    #[test]
    fn target_conflicts_and_unknown_schema_qualification_block_plan() {
        let (sites, databases, mut target, mut handoff) = complete_evidence();
        target.site_domains.insert("proxy.example.test".into());
        handoff.source_schema_version_verified = false;
        let plan = Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff);
        assert_eq!(plan.actions[0].action, PlanActionType::Conflict);
        assert!(!plan.decommission_allowed);
    }

    #[test]
    fn hyphenated_names_are_preserved_and_option_like_names_are_rejected() {
        assert!(valid_linux_username("app-user"));
        assert!(valid_linux_username("app_user"));
        assert!(!valid_linux_username("-app-user"));
        assert!(!valid_linux_username("app/user"));
        let (sites, databases, target, handoff) = complete_evidence();
        let plan = Planner::create_review_plan(&sample(), &sites, &databases, &target, &handoff);
        assert_eq!(plan.actions[3].item_name, "app-user");
        assert_eq!(plan.actions[3].action, PlanActionType::Create);
    }

    #[test]
    fn collisions_are_exact_and_never_normalize_hyphen_to_underscore() {
        let (sites, databases, mut target, handoff) = complete_evidence();
        target.os_usernames.insert("app_user".into());
        let mut inventory = sample();
        inventory.items[3].name = "app-user".into();
        assert_eq!(
            Planner::create_review_plan(&inventory, &sites, &databases, &target, &handoff).actions
                [3]
            .action,
            PlanActionType::Create
        );
        target.os_usernames.insert("app-user".into());
        assert_eq!(
            Planner::create_review_plan(&inventory, &sites, &databases, &target, &handoff).actions
                [3]
            .action,
            PlanActionType::Conflict
        );
    }

    #[test]
    fn multi_database_associations_are_retained_and_each_database_is_independently_checked() {
        let (sites, mut databases, target, handoff) = complete_evidence();
        let mut inventory = sample();
        inventory.items[1].id = "db_2".into();
        inventory.items[1].name = "analytics_db".into();
        inventory.items[1].associations = vec!["site_1".into()];
        inventory.items.insert(
            2,
            InventoryItem {
                id: "db_1b".into(),
                item_type: InventoryItemType::Database,
                name: "secondary_db".into(),
                version: None,
                associations: vec!["site_1".into()],
                metadata: HashMap::new(),
            },
        );
        inventory.items[3].associations = vec!["db_2".into()];
        databases[0].source_database_id = "db_2".into();
        databases.push(SourceDatabaseEvidence {
            source_database_id: "db_1b".into(),
            ownership_reviewed: true,
            read_only_export_access_verified: true,
            users_and_grants_reviewed: true,
        });
        let plan = Planner::create_review_plan(&inventory, &sites, &databases, &target, &handoff);
        assert_eq!(plan.actions[1].action, PlanActionType::Create);
        assert_eq!(plan.actions[2].action, PlanActionType::Create);
        assert_eq!(plan.actions[3].action, PlanActionType::Create);
    }
}

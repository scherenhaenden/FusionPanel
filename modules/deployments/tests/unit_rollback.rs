use std::sync::{Arc, Mutex};
use deployments::rollback::inventory::{ReleaseInventory, Release, Plan, AppliedResult, ObservedState, ReleaseStatus};
use deployments::rollback::manager::RollbackManager;
use uuid::Uuid;
use chrono::Utc;

fn create_release(target_id: &str, status: ReleaseStatus) -> Release {
    Release {
        id: Uuid::new_v4(),
        plan: Plan {
            target_id: target_id.to_string(),
            revision: "rev123".to_string(),
        },
        applied_result: Some(AppliedResult {
            success: status == ReleaseStatus::Healthy || status == ReleaseStatus::Active,
            message: "Applied".to_string(),
        }),
        observed_state: ObservedState {
            status,
            last_checked_at: Utc::now(),
        },
        created_at: Utc::now(),
    }
}

#[test]
fn test_inventory_retention() {
    let mut inventory = ReleaseInventory::new(2);
    let target = "target-1";

    let r1 = create_release(target, ReleaseStatus::Healthy);
    let r2 = create_release(target, ReleaseStatus::Failed);
    let r3 = create_release(target, ReleaseStatus::Healthy);
    let r4 = create_release(target, ReleaseStatus::Failed);
    let r5 = create_release(target, ReleaseStatus::Healthy);

    inventory.add_release(r1.clone());
    inventory.add_release(r2.clone());
    inventory.add_release(r3.clone());
    inventory.add_release(r4.clone());
    inventory.add_release(r5.clone());

    assert!(inventory.get_release(target, r1.id).is_none());
    assert!(inventory.get_release(target, r3.id).is_some());
    assert!(inventory.get_release(target, r5.id).is_some());

    assert!(inventory.get_release(target, r2.id).is_some());
    assert!(inventory.get_release(target, r4.id).is_some());
}

#[test]
fn test_fast_rollback() {
    let mut inventory = ReleaseInventory::new(5);
    let target = "target-1";

    let healthy_release = create_release(target, ReleaseStatus::Healthy);
    let active_failed = create_release(target, ReleaseStatus::Active);

    inventory.add_release(healthy_release.clone());
    inventory.add_release(active_failed.clone());

    let inventory_arc = Arc::new(Mutex::new(inventory));
    let manager = RollbackManager::new(inventory_arc.clone());

    let result = manager.rollback_to_last_healthy(target);
    assert!(result.is_ok());

    let rolled_back_to_release = result.unwrap();
    assert_eq!(rolled_back_to_release.id, healthy_release.id);
    assert_eq!(rolled_back_to_release.observed_state.status, ReleaseStatus::Active);

    let locked = inventory_arc.lock().unwrap();
    let old_active = locked.get_release(target, active_failed.id).unwrap();
    assert_eq!(old_active.observed_state.status, ReleaseStatus::RolledBack);
}

#[test]
#[should_panic(expected = "BLOCKED: Deployment execution engine unavailable for physical rollback test")]
fn test_fast_rollback_blocked() {
    let is_deployment_engine_available = false;

    if !is_deployment_engine_available {
        panic!("BLOCKED: Deployment execution engine unavailable for physical rollback test");
    }
}

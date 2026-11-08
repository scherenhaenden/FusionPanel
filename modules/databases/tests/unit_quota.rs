#![forbid(unsafe_code)]

use databases::quota::{QuotaEnforcer, QuotaError, QuotaLimits, QuotaUsage};
use uuid::Uuid;

#[test]
fn test_set_and_get_limits() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    let limits = QuotaLimits {
        max_storage_bytes: 1024,
        max_table_count: 10,
    };

    enforcer.set_limits(tenant_id, limits.clone());

    let retrieved_limits = enforcer.get_limits(tenant_id).unwrap();
    assert_eq!(retrieved_limits.max_storage_bytes, 1024);
    assert_eq!(retrieved_limits.max_table_count, 10);
}

#[test]
fn test_set_and_get_usage() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    let usage = QuotaUsage {
        storage_bytes: 512,
        table_count: 5,
    };

    enforcer.set_usage(tenant_id, usage.clone());

    let retrieved_usage = enforcer.get_usage(tenant_id).unwrap();
    assert_eq!(retrieved_usage.storage_bytes, 512);
    assert_eq!(retrieved_usage.table_count, 5);
}

#[test]
fn test_allocation_success() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    enforcer.set_limits(
        tenant_id,
        QuotaLimits {
            max_storage_bytes: 1024,
            max_table_count: 10,
        },
    );

    assert!(enforcer.allocate(tenant_id, 512, 5).is_ok());

    let usage = enforcer.get_usage(tenant_id).unwrap();
    assert_eq!(usage.storage_bytes, 512);
    assert_eq!(usage.table_count, 5);
}

#[test]
fn test_allocation_storage_exceeded() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    enforcer.set_limits(
        tenant_id,
        QuotaLimits {
            max_storage_bytes: 1024,
            max_table_count: 10,
        },
    );

    let result = enforcer.allocate(tenant_id, 2048, 5);
    assert!(matches!(result, Err(QuotaError::StorageExceeded { .. })));
}

#[test]
fn test_allocation_table_count_exceeded() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    enforcer.set_limits(
        tenant_id,
        QuotaLimits {
            max_storage_bytes: 1024,
            max_table_count: 10,
        },
    );

    let result = enforcer.allocate(tenant_id, 512, 15);
    assert!(matches!(result, Err(QuotaError::TableCountExceeded { .. })));
}

#[test]
fn test_release_allocation() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    enforcer.set_usage(
        tenant_id,
        QuotaUsage {
            storage_bytes: 1024,
            table_count: 10,
        },
    );

    assert!(enforcer.release(tenant_id, 512, 5).is_ok());

    let usage = enforcer.get_usage(tenant_id).unwrap();
    assert_eq!(usage.storage_bytes, 512);
    assert_eq!(usage.table_count, 5);
}

#[test]
fn test_release_allocation_saturating() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    enforcer.set_usage(
        tenant_id,
        QuotaUsage {
            storage_bytes: 512,
            table_count: 5,
        },
    );

    assert!(enforcer.release(tenant_id, 1024, 10).is_ok());

    let usage = enforcer.get_usage(tenant_id).unwrap();
    assert_eq!(usage.storage_bytes, 0);
    assert_eq!(usage.table_count, 0);
}

#[test]
fn test_calculate_percentages() {
    let mut enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    enforcer.set_limits(
        tenant_id,
        QuotaLimits {
            max_storage_bytes: 1024,
            max_table_count: 10,
        },
    );

    enforcer.set_usage(
        tenant_id,
        QuotaUsage {
            storage_bytes: 512,
            table_count: 2,
        },
    );

    let percentages = enforcer.calculate_usage_percentages(tenant_id).unwrap();
    assert_eq!(percentages.storage_percentage, 50.0);
    assert_eq!(percentages.table_count_percentage, 20.0);
}

#[test]
fn test_tenant_not_found() {
    let enforcer = QuotaEnforcer::new();
    let tenant_id = Uuid::new_v4();

    assert!(matches!(
        enforcer.get_usage(tenant_id),
        Err(QuotaError::TenantNotFound(_))
    ));
}

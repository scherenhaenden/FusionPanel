#![forbid(unsafe_code)]

use administration::quota_guard::{
    QuotaViolation, ResourceRequest, TenantCurrentUsage, TenantQuotaGuard, TenantQuotaLimits,
};

#[test]
fn test_within_quota_approval() {
    let limits = TenantQuotaLimits {
        max_domains: 5,
        max_databases: 3,
        max_mailboxes: 10,
        max_storage_bytes: 1000,
        max_memory_bytes: 2000,
    };

    let usage = TenantCurrentUsage {
        domain_count: 2,
        database_count: 1,
        mailbox_count: 5,
        storage_bytes: 500,
        memory_bytes: 1000,
    };

    let request = ResourceRequest {
        domains: 2,
        databases: 1,
        mailboxes: 4,
        storage_bytes: 400,
        memory_bytes: 800,
    };

    let result = TenantQuotaGuard::evaluate(&limits, &usage, &request);
    assert!(result.is_ok());
}

#[test]
fn test_single_resource_violation() {
    let limits = TenantQuotaLimits {
        max_domains: 5,
        max_databases: 3,
        max_mailboxes: 10,
        max_storage_bytes: 1000,
        max_memory_bytes: 2000,
    };

    let usage = TenantCurrentUsage {
        domain_count: 2,
        database_count: 1,
        mailbox_count: 5,
        storage_bytes: 500,
        memory_bytes: 1000,
    };

    let request = ResourceRequest {
        domains: 4, // 2 + 4 = 6 > 5
        databases: 1,
        mailboxes: 4,
        storage_bytes: 400,
        memory_bytes: 800,
    };

    let result = TenantQuotaGuard::evaluate(&limits, &usage, &request);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.violations.len(), 1);
    assert_eq!(
        err.violations[0],
        QuotaViolation {
            resource: "domains".to_string(),
            requested_total: 6,
            limit: 5,
            remaining: 3,
        }
    );
}

#[test]
fn test_multi_resource_violation() {
    let limits = TenantQuotaLimits {
        max_domains: 5,
        max_databases: 3,
        max_mailboxes: 10,
        max_storage_bytes: 1000,
        max_memory_bytes: 2000,
    };

    let usage = TenantCurrentUsage {
        domain_count: 2,
        database_count: 1,
        mailbox_count: 5,
        storage_bytes: 500,
        memory_bytes: 1000,
    };

    let request = ResourceRequest {
        domains: 4,     // 2 + 4 = 6 > 5
        databases: 3,   // 1 + 3 = 4 > 3
        mailboxes: 4,
        storage_bytes: 600, // 500 + 600 = 1100 > 1000
        memory_bytes: 800,
    };

    let result = TenantQuotaGuard::evaluate(&limits, &usage, &request);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.violations.len(), 3);

    assert!(err.violations.contains(&QuotaViolation {
        resource: "domains".to_string(),
        requested_total: 6,
        limit: 5,
        remaining: 3,
    }));

    assert!(err.violations.contains(&QuotaViolation {
        resource: "databases".to_string(),
        requested_total: 4,
        limit: 3,
        remaining: 2,
    }));

    assert!(err.violations.contains(&QuotaViolation {
        resource: "storage_bytes".to_string(),
        requested_total: 1100,
        limit: 1000,
        remaining: 500,
    }));
}

#[test]
fn test_zero_quota_edge_cases() {
    let limits = TenantQuotaLimits {
        max_domains: 0,
        max_databases: 0,
        max_mailboxes: 0,
        max_storage_bytes: 0,
        max_memory_bytes: 0,
    };

    let usage = TenantCurrentUsage {
        domain_count: 0,
        database_count: 0,
        mailbox_count: 0,
        storage_bytes: 0,
        memory_bytes: 0,
    };

    // Requesting zero should be ok
    let request_zero = ResourceRequest {
        domains: 0,
        databases: 0,
        mailboxes: 0,
        storage_bytes: 0,
        memory_bytes: 0,
    };

    assert!(TenantQuotaGuard::evaluate(&limits, &usage, &request_zero).is_ok());

    // Requesting anything should fail
    let request_one = ResourceRequest {
        domains: 1,
        databases: 0,
        mailboxes: 0,
        storage_bytes: 0,
        memory_bytes: 0,
    };

    let result = TenantQuotaGuard::evaluate(&limits, &usage, &request_one);
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.violations.len(), 1);
    assert_eq!(
        err.violations[0],
        QuotaViolation {
            resource: "domains".to_string(),
            requested_total: 1,
            limit: 0,
            remaining: 0,
        }
    );
}

#[test]
fn test_remaining_capacity() {
    let limits = TenantQuotaLimits {
        max_domains: 5,
        max_databases: 3,
        max_mailboxes: 10,
        max_storage_bytes: 1000,
        max_memory_bytes: 2000,
    };

    let usage = TenantCurrentUsage {
        domain_count: 2,
        database_count: 1,
        mailbox_count: 15, // Over quota
        storage_bytes: 500,
        memory_bytes: 1000,
    };

    let remaining = TenantQuotaGuard::remaining_capacity(&limits, &usage);
    assert_eq!(
        remaining,
        ResourceRequest {
            domains: 3,
            databases: 2,
            mailboxes: 0, // Saturating sub correctly handled over quota
            storage_bytes: 500,
            memory_bytes: 1000,
        }
    );
}

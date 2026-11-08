#![forbid(unsafe_code)]

use administration::quotas::{QuotaEngine, QuotaError, ResourceQuotas};
use uuid::Uuid;

#[test]
fn test_default_quotas() {
    let q = ResourceQuotas::default();
    assert_eq!(q.cpu_cores, 0);
    assert_eq!(q.memory_mb, 0);
    assert_eq!(q.disk_mb, 0);
    assert_eq!(q.domain_count, 0);
    assert_eq!(q.database_count, 0);
}

#[test]
fn test_quota_addition_and_subtraction() {
    let mut q1 = ResourceQuotas {
        cpu_cores: 2,
        memory_mb: 1024,
        disk_mb: 10000,
        domain_count: 5,
        database_count: 2,
    };

    let q2 = ResourceQuotas {
        cpu_cores: 1,
        memory_mb: 512,
        disk_mb: 5000,
        domain_count: 2,
        database_count: 1,
    };

    q1.add(&q2);
    assert_eq!(q1.cpu_cores, 3);
    assert_eq!(q1.memory_mb, 1536);
    assert_eq!(q1.disk_mb, 15000);
    assert_eq!(q1.domain_count, 7);
    assert_eq!(q1.database_count, 3);

    q1.sub(&q2);
    assert_eq!(q1.cpu_cores, 2);
    assert_eq!(q1.memory_mb, 1024);
    assert_eq!(q1.disk_mb, 10000);
    assert_eq!(q1.domain_count, 5);
    assert_eq!(q1.database_count, 2);

    // Test saturating subtraction
    let q3 = ResourceQuotas {
        cpu_cores: 5,
        memory_mb: 2048,
        disk_mb: 20000,
        domain_count: 10,
        database_count: 5,
    };
    q1.sub(&q3);
    assert_eq!(q1.cpu_cores, 0);
    assert_eq!(q1.memory_mb, 0);
    assert_eq!(q1.disk_mb, 0);
    assert_eq!(q1.domain_count, 0);
    assert_eq!(q1.database_count, 0);
}

#[test]
fn test_quota_engine_reservation_success() {
    let engine = QuotaEngine::new();
    let tenant_id = Uuid::new_v4();

    engine.set_limits(
        tenant_id,
        ResourceQuotas {
            cpu_cores: 4,
            memory_mb: 4096,
            disk_mb: 50000,
            domain_count: 10,
            database_count: 5,
        },
    );

    let request = ResourceQuotas {
        cpu_cores: 2,
        memory_mb: 2048,
        disk_mb: 10000,
        domain_count: 1,
        database_count: 1,
    };

    let result = engine.reserve(tenant_id, &request);
    assert!(result.is_ok());

    let usage = engine.get_usage(tenant_id);
    assert_eq!(usage, request);

    // Make another successful reservation
    let request2 = ResourceQuotas {
        cpu_cores: 1,
        memory_mb: 1024,
        disk_mb: 5000,
        domain_count: 2,
        database_count: 1,
    };

    let result2 = engine.reserve(tenant_id, &request2);
    assert!(result2.is_ok());

    let usage = engine.get_usage(tenant_id);
    assert_eq!(usage.cpu_cores, 3);
    assert_eq!(usage.memory_mb, 3072);
}

#[test]
fn test_quota_engine_reservation_failure() {
    let engine = QuotaEngine::new();
    let tenant_id = Uuid::new_v4();

    engine.set_limits(
        tenant_id,
        ResourceQuotas {
            cpu_cores: 2,
            memory_mb: 2048,
            disk_mb: 10000,
            domain_count: 2,
            database_count: 1,
        },
    );

    let request = ResourceQuotas {
        cpu_cores: 3,
        memory_mb: 1024,
        disk_mb: 5000,
        domain_count: 1,
        database_count: 1,
    };

    let result = engine.reserve(tenant_id, &request);
    assert_eq!(result, Err(QuotaError::LimitExceeded));

    // Ensure usage remains 0 after failure
    let usage = engine.get_usage(tenant_id);
    assert_eq!(usage, ResourceQuotas::default());
}

#[test]
fn test_quota_engine_unconfigured_tenant() {
    let engine = QuotaEngine::new();
    let tenant_id = Uuid::new_v4();

    let request = ResourceQuotas {
        cpu_cores: 1,
        memory_mb: 512,
        disk_mb: 1000,
        domain_count: 1,
        database_count: 1,
    };

    let result = engine.reserve(tenant_id, &request);
    assert_eq!(result, Err(QuotaError::LimitsNotConfigured));
}

#[test]
fn test_quota_engine_release() {
    let engine = QuotaEngine::new();
    let tenant_id = Uuid::new_v4();

    engine.set_limits(
        tenant_id,
        ResourceQuotas {
            cpu_cores: 4,
            memory_mb: 4096,
            disk_mb: 50000,
            domain_count: 10,
            database_count: 5,
        },
    );

    let request = ResourceQuotas {
        cpu_cores: 2,
        memory_mb: 2048,
        disk_mb: 10000,
        domain_count: 2,
        database_count: 1,
    };

    engine.reserve(tenant_id, &request).unwrap();
    assert_eq!(engine.get_usage(tenant_id), request);

    let release_req = ResourceQuotas {
        cpu_cores: 1,
        memory_mb: 1024,
        disk_mb: 5000,
        domain_count: 1,
        database_count: 0,
    };

    engine.release(tenant_id, &release_req);

    let usage = engine.get_usage(tenant_id);
    assert_eq!(usage.cpu_cores, 1);
    assert_eq!(usage.memory_mb, 1024);
    assert_eq!(usage.disk_mb, 5000);
    assert_eq!(usage.domain_count, 1);
    assert_eq!(usage.database_count, 1);
}

#[test]
fn test_atomic_concurrent_reservations() {
    use std::sync::Arc;
    use std::thread;

    let engine = Arc::new(QuotaEngine::new());
    let tenant_id = Uuid::new_v4();

    engine.set_limits(
        tenant_id,
        ResourceQuotas {
            cpu_cores: 10,
            memory_mb: 10240,
            disk_mb: 100000,
            domain_count: 20,
            database_count: 10,
        },
    );

    let mut handles = vec![];

    // Spawn 10 threads, each reserving 1 CPU core
    for _ in 0..10 {
        let engine_clone = Arc::clone(&engine);
        let handle = thread::spawn(move || {
            let req = ResourceQuotas {
                cpu_cores: 1,
                memory_mb: 1024,
                disk_mb: 10000,
                domain_count: 2,
                database_count: 1,
            };
            engine_clone.reserve(tenant_id, &req)
        });
        handles.push(handle);
    }

    let mut success_count = 0;
    for handle in handles {
        if let Ok(Ok(_)) = handle.join() {
            success_count += 1;
        }
    }

    assert_eq!(success_count, 10);

    let usage = engine.get_usage(tenant_id);
    assert_eq!(usage.cpu_cores, 10);
    assert_eq!(usage.memory_mb, 10240);

    // An 11th request should fail
    let req_fail = ResourceQuotas {
        cpu_cores: 1,
        memory_mb: 1024,
        disk_mb: 10000,
        domain_count: 2,
        database_count: 1,
    };
    assert_eq!(
        engine.reserve(tenant_id, &req_fail),
        Err(QuotaError::LimitExceeded)
    );
}

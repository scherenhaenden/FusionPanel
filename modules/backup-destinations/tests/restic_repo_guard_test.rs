#![forbid(unsafe_code)]

use backup_destinations::restic_repo_guard::{LockError, LockType, ResticLock, ResticLockGuard};
use chrono::{Duration, Utc};
use std::time::Duration as StdDuration;

#[test]
fn test_parse_shared_lock() {
    let json_data = r#"{
        "time": "2023-10-01T12:00:00Z",
        "exclusive": false,
        "hostname": "backup-server",
        "username": "root",
        "pid": 1234,
        "uid": 0,
        "gid": 0
    }"#;
    let now = Utc::now();
    let lock = ResticLockGuard::parse_lock("lock1", json_data, now).unwrap();

    assert_eq!(lock.id, "lock1");
    assert_eq!(lock.lock_type, LockType::Shared);
    assert_eq!(lock.hostname, "backup-server");
    assert_eq!(lock.pid, 1234);
}

#[test]
fn test_parse_exclusive_lock() {
    let json_data = r#"{
        "time": "2023-10-01T12:00:00Z",
        "exclusive": true,
        "hostname": "backup-server",
        "username": "root",
        "pid": 5678,
        "uid": 0,
        "gid": 0
    }"#;
    let now = Utc::now();
    let lock = ResticLockGuard::parse_lock("lock2", json_data, now).unwrap();

    assert_eq!(lock.id, "lock2");
    assert_eq!(lock.lock_type, LockType::Exclusive);
}

#[test]
fn test_is_stale() {
    let now = Utc::now();
    let updated_at = now - Duration::hours(7);

    let lock = ResticLock {
        id: "stale-lock".to_string(),
        lock_type: LockType::Shared,
        created_at: updated_at,
        updated_at,
        pid: 111,
        hostname: "host".to_string(),
    };

    assert!(lock.is_stale(now, StdDuration::from_secs(6 * 3600)));
    assert!(!lock.is_stale(now, StdDuration::from_secs(8 * 3600)));
}

#[test]
fn test_load_lock_cleans_stale() {
    let mut guard = ResticLockGuard::new(StdDuration::from_secs(3600));
    let now = Utc::now();

    let stale_lock = ResticLock {
        id: "stale".to_string(),
        lock_type: LockType::Shared,
        created_at: now - Duration::hours(2),
        updated_at: now - Duration::hours(2),
        pid: 1,
        hostname: "h1".to_string(),
    };

    let fresh_lock = ResticLock {
        id: "fresh".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 2,
        hostname: "h2".to_string(),
    };

    guard.load_lock(stale_lock, now);
    assert_eq!(guard.get_locks().len(), 0);

    guard.load_lock(fresh_lock, now);
    assert_eq!(guard.get_locks().len(), 1);
}

#[test]
fn test_acquire_shared_locks() {
    let mut guard = ResticLockGuard::new(StdDuration::from_secs(3600));
    let now = Utc::now();

    let lock1 = ResticLock {
        id: "s1".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 1,
        hostname: "h1".to_string(),
    };

    let lock2 = ResticLock {
        id: "s2".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 2,
        hostname: "h2".to_string(),
    };

    assert!(guard.acquire_lock(lock1, now).is_ok());
    assert!(guard.acquire_lock(lock2, now).is_ok());
    assert_eq!(guard.get_locks().len(), 2);
}

#[test]
fn test_acquire_exclusive_conflict() {
    let mut guard = ResticLockGuard::new(StdDuration::from_secs(3600));
    let now = Utc::now();

    let exc_lock = ResticLock {
        id: "e1".to_string(),
        lock_type: LockType::Exclusive,
        created_at: now,
        updated_at: now,
        pid: 1,
        hostname: "h1".to_string(),
    };

    let shared_lock = ResticLock {
        id: "s1".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 2,
        hostname: "h2".to_string(),
    };

    assert!(guard.acquire_lock(exc_lock, now).is_ok());
    assert_eq!(
        guard.acquire_lock(shared_lock, now),
        Err(LockError::ExclusiveLockConflict("e1".to_string()))
    );
}

#[test]
fn test_acquire_exclusive_with_existing_shared() {
    let mut guard = ResticLockGuard::new(StdDuration::from_secs(3600));
    let now = Utc::now();

    let shared_lock = ResticLock {
        id: "s1".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 2,
        hostname: "h2".to_string(),
    };

    let exc_lock = ResticLock {
        id: "e1".to_string(),
        lock_type: LockType::Exclusive,
        created_at: now,
        updated_at: now,
        pid: 1,
        hostname: "h1".to_string(),
    };

    assert!(guard.acquire_lock(shared_lock, now).is_ok());
    assert_eq!(
        guard.acquire_lock(exc_lock, now),
        Err(LockError::SharedLockConflict)
    );
}

#[test]
fn test_release_lock_success() {
    let mut guard = ResticLockGuard::new(StdDuration::from_secs(3600));
    let now = Utc::now();

    let lock = ResticLock {
        id: "l1".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 123,
        hostname: "myhost".to_string(),
    };

    assert!(guard.acquire_lock(lock, now).is_ok());
    assert!(guard.release_lock("l1", "myhost", 123).is_ok());
    assert_eq!(guard.get_locks().len(), 0);
}

#[test]
fn test_release_lock_unauthorized() {
    let mut guard = ResticLockGuard::new(StdDuration::from_secs(3600));
    let now = Utc::now();

    let lock = ResticLock {
        id: "l1".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 123,
        hostname: "myhost".to_string(),
    };

    assert!(guard.acquire_lock(lock, now).is_ok());

    // Wrong PID
    assert_eq!(
        guard.release_lock("l1", "myhost", 999),
        Err(LockError::UnauthorizedUnlock("l1".to_string()))
    );

    // Wrong hostname
    assert_eq!(
        guard.release_lock("l1", "otherhost", 123),
        Err(LockError::UnauthorizedUnlock("l1".to_string()))
    );
}

#[test]
fn test_refresh_lock() {
    let mut guard = ResticLockGuard::new(StdDuration::from_secs(3600));
    let now = Utc::now();
    let later = now + Duration::minutes(30);

    let lock = ResticLock {
        id: "l1".to_string(),
        lock_type: LockType::Shared,
        created_at: now,
        updated_at: now,
        pid: 123,
        hostname: "myhost".to_string(),
    };

    assert!(guard.acquire_lock(lock, now).is_ok());
    assert!(guard.refresh_lock("l1", "myhost", 123, later).is_ok());

    let locks = guard.get_locks();
    assert_eq!(locks.len(), 1);
    assert_eq!(locks[0].updated_at, later);
}

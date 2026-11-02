use chrono::{Duration, Utc};
use identity::{
    AccessControl, IdentityError, PasswordManager, Permission, Role, RoleBinding, Scope, Session,
    SqliteIdentityStore, TenantId, User, UserId, UserStatus,
};
use tempfile::NamedTempFile;

#[test]
fn test_user_creation_and_reopen_persistence() {
    let tmp_file = NamedTempFile::new().unwrap();
    let db_path = tmp_file.path().to_path_buf();

    let raw_password = "SecureAdminPassword2026!";
    let password_hash = PasswordManager::hash_password(raw_password).unwrap();

    let user = User::new(
        "alice_admin".to_string(),
        "alice@example.com".to_string(),
        password_hash.clone(),
    )
    .unwrap();
    let user_id = user.id();

    // 1. Write to database and close connection
    {
        let store = SqliteIdentityStore::open(&db_path).unwrap();
        store.create_user(&user).unwrap();

        // Verify duplicate username is rejected with Conflict
        let dup = User::new(
            "alice_admin".to_string(),
            "another@example.com".to_string(),
            password_hash.clone(),
        )
        .unwrap();
        assert!(matches!(
            store.create_user(&dup),
            Err(IdentityError::Conflict { .. })
        ));
    } // Store is dropped here, closing SQLite connection

    // 2. Reopen from disk and verify persistence
    {
        let store_reopened = SqliteIdentityStore::open(&db_path).unwrap();

        let fetched = store_reopened.get_user(user_id).unwrap();
        assert!(fetched.is_some());
        let fetched_user = fetched.unwrap();

        assert_eq!(fetched_user.id(), user_id);
        assert_eq!(fetched_user.username(), "alice_admin");
        assert_eq!(fetched_user.email(), "alice@example.com");
        assert_eq!(fetched_user.status(), UserStatus::Active);

        // Secret-safe: hash is stored, never plaintext password
        assert_eq!(fetched_user.password_hash(), &password_hash);
        assert_ne!(fetched_user.password_hash(), raw_password);
        assert!(
            PasswordManager::verify_password(raw_password, fetched_user.password_hash()).is_ok()
        );

        // Also test query by username
        let by_name = store_reopened
            .get_user_by_username("alice_admin")
            .unwrap()
            .unwrap();
        assert_eq!(by_name.id(), user_id);
    }
}

#[test]
fn test_session_persistence_lifecycle_and_reopen() {
    let tmp_file = NamedTempFile::new().unwrap();
    let db_path = tmp_file.path().to_path_buf();

    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();

    let session = Session::new(user_id, tenant_id, 3600, 600).unwrap();
    let session_id = session.id();

    // 1. Create and touch session
    {
        let store = SqliteIdentityStore::open(&db_path).unwrap();
        store.save_session(&session).unwrap();

        let touch_time = Utc::now() + Duration::seconds(120);
        store.update_session_touch(session_id, touch_time).unwrap();
    }

    // 2. Reopen and verify touched timestamp persisted
    {
        let store_reopened = SqliteIdentityStore::open(&db_path).unwrap();
        let loaded = store_reopened.get_session(session_id).unwrap().unwrap();

        assert_eq!(loaded.id(), session_id);
        assert_eq!(loaded.user_id(), user_id);
        assert_eq!(loaded.tenant_id(), tenant_id);
        assert!(!loaded.is_revoked());
        assert!(loaded.last_accessed_at() > loaded.created_at());

        // Revoke session
        store_reopened
            .revoke_session(session_id, "Security audit logout")
            .unwrap();
    }

    // 3. Reopen again and verify revocation persisted
    {
        let store_reopened2 = SqliteIdentityStore::open(&db_path).unwrap();
        let loaded = store_reopened2.get_session(session_id).unwrap().unwrap();

        assert!(loaded.is_revoked());
        assert_eq!(loaded.revocation_reason(), Some("Security audit logout"));

        // Touching a revoked session must fail
        let touch_res = store_reopened2.update_session_touch(session_id, Utc::now());
        assert!(matches!(
            touch_res,
            Err(IdentityError::SessionRevoked { .. })
        ));
    }
}

#[test]
fn test_multi_tenant_isolation_in_storage() {
    let store = SqliteIdentityStore::in_memory().unwrap();

    let tenant_a = TenantId::new_v4();
    let tenant_b = TenantId::new_v4();

    let user_a = UserId::new_v4();
    let user_b = UserId::new_v4();

    let s_a1 = Session::new(user_a, tenant_a, 3600, 900).unwrap();
    let s_a2 = Session::new(user_a, tenant_a, 3600, 900).unwrap();
    let s_b1 = Session::new(user_b, tenant_b, 3600, 900).unwrap();

    store.save_session(&s_a1).unwrap();
    store.save_session(&s_a2).unwrap();
    store.save_session(&s_b1).unwrap();

    // Query Tenant A sessions: must contain ONLY tenant A's sessions
    let sessions_a = store.list_tenant_sessions(tenant_a).unwrap();
    assert_eq!(sessions_a.len(), 2);
    for s in &sessions_a {
        assert_eq!(s.tenant_id(), tenant_a);
    }

    // Query Tenant B sessions: must contain ONLY tenant B's session
    let sessions_b = store.list_tenant_sessions(tenant_b).unwrap();
    assert_eq!(sessions_b.len(), 1);
    assert_eq!(sessions_b[0].id(), s_b1.id());
    assert_eq!(sessions_b[0].tenant_id(), tenant_b);
}

#[test]
fn test_bulk_revocation_registry_in_storage() {
    let store = SqliteIdentityStore::in_memory().unwrap();

    let user_1 = UserId::new_v4();
    let tenant_1 = TenantId::new_v4();

    let s1 = Session::new(user_1, tenant_1, 3600, 900).unwrap();
    store.save_session(&s1).unwrap();

    assert!(!store.is_revoked(&s1).unwrap());

    // Record bulk user revocation in store
    store
        .record_revocation("user", &user_1.to_string(), "Password reset")
        .unwrap();

    assert!(store.is_revoked(&s1).unwrap());
}

#[test]
fn test_role_bindings_storage_and_evaluation() {
    let store = SqliteIdentityStore::in_memory().unwrap();

    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();

    let binding = RoleBinding {
        user_id,
        role: Role::tenant_admin(),
        scope: Scope::Tenant { tenant_id },
    };

    store.save_role_binding(&binding).unwrap();

    let loaded_bindings = store.get_user_role_bindings(user_id).unwrap();
    assert_eq!(loaded_bindings.len(), 1);
    assert_eq!(loaded_bindings[0].user_id, user_id);

    // Evaluate access using principal
    assert!(AccessControl::evaluate_principal(
        user_id,
        &loaded_bindings,
        Permission::SiteCreate,
        &Scope::Tenant { tenant_id }
    )
    .is_ok());

    // Evaluate access for unauthorized permission
    assert!(matches!(
        AccessControl::evaluate_principal(
            user_id,
            &loaded_bindings,
            Permission::SystemAdmin,
            &Scope::Tenant { tenant_id }
        ),
        Err(IdentityError::Unauthorized { .. })
    ));
}

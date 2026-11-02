use chrono::Utc;
use identity::error::IdentityError;
use identity::rbac::{AccessControl, Permission, Role, Scope};
use identity::session::Session;
use identity::storage::IdentityStorage;
use identity::types::{SiteId, TenantId, User, UserId, UserStatus};
use tempfile::NamedTempFile;

#[test]
fn test_real_sqlite_persistence_reopen() {
    let temp_db = NamedTempFile::new().expect("Failed to create temp file");
    let db_path = temp_db.path();

    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();
    let site_id = SiteId::new_v4();

    // 1. Initial write pass
    {
        let mut storage =
            IdentityStorage::open(db_path).expect("Failed to open SQLite database for writing");

        let user = User {
            id: user_id,
            username: "alice_admin".to_string(),
            email: "alice@example.com".to_string(),
            status: UserStatus::Active,
            password_hash: "$argon2id$v=19$m=19456,t=2,p=1$fakehash".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        storage.insert_user(&user).expect("Failed to insert user");

        let role = Role::tenant_admin();
        let role_id = role.id;
        storage.insert_role(&role).expect("Failed to insert role");

        storage
            .bind_role(user_id, role_id, Scope::Site { tenant_id, site_id })
            .expect("Failed to bind role");

        let session =
            Session::new(user_id, tenant_id, 3600, 300).expect("Failed to create session");
        let session_id = session.id;
        storage
            .insert_session(&session)
            .expect("Failed to insert session");

        // Revoke the session
        storage
            .revoke_session(session_id, "Manual logout")
            .expect("Failed to revoke session");
    }

    // 2. Reopen pass: Ensure data survives process/handle recreation
    {
        let storage = IdentityStorage::open(db_path).expect("Failed to reopen SQLite database");

        // Verify User persisted
        let fetched_user = storage
            .get_user(user_id)
            .expect("User not found after reopen");
        assert_eq!(fetched_user.username, "alice_admin");
        assert_eq!(fetched_user.email, "alice@example.com");
        assert_eq!(fetched_user.status, UserStatus::Active);

        // Verify Role & Bindings persisted
        let bindings = storage
            .get_user_bindings(user_id)
            .expect("Failed to get user bindings after reopen");
        assert_eq!(bindings.len(), 1);
        assert_eq!(bindings[0].scope, Scope::Site { tenant_id, site_id });

        // Evaluate permissions using persisted bindings
        let site_scope = Scope::Site { tenant_id, site_id };
        assert!(AccessControl::evaluate(&bindings, Permission::SiteDeploy, &site_scope).is_ok());

        // Negative: Cannot deploy on non-matching site
        let other_site = Scope::Site {
            tenant_id,
            site_id: SiteId::new_v4(),
        };
        assert!(matches!(
            AccessControl::evaluate(&bindings, Permission::SiteDeploy, &other_site),
            Err(IdentityError::Unauthorized { .. })
        ));
    }
}

#[test]
fn test_independent_tenant_contexts_and_isolation() {
    let temp_db = NamedTempFile::new().expect("Failed to create temp file");
    let db_path = temp_db.path();

    let tenant_one = TenantId::new_v4();
    let tenant_two = TenantId::new_v4();

    let user_one = UserId::new_v4();
    let user_two = UserId::new_v4();

    let mut storage = IdentityStorage::open(db_path).expect("Failed to open SQLite database");

    // Create user 1 in tenant 1
    let u1 = User {
        id: user_one,
        username: "tenant1_user".to_string(),
        email: "u1@tenant1.com".to_string(),
        status: UserStatus::Active,
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$fake1".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    storage.insert_user(&u1).unwrap();

    // Create user 2 in tenant 2
    let u2 = User {
        id: user_two,
        username: "tenant2_user".to_string(),
        email: "u2@tenant2.com".to_string(),
        status: UserStatus::Active,
        password_hash: "$argon2id$v=19$m=19456,t=2,p=1$fake2".to_string(),
        created_at: Utc::now(),
        updated_at: Utc::now(),
    };
    storage.insert_user(&u2).unwrap();

    let role_t1 = Role::tenant_admin();
    let role_t1_id = role_t1.id;
    storage.insert_role(&role_t1).unwrap();

    storage
        .bind_role(
            user_one,
            role_t1_id,
            Scope::Tenant {
                tenant_id: tenant_one,
            },
        )
        .unwrap();

    let bindings_u1 = storage.get_user_bindings(user_one).unwrap();

    // User 1 has access to Tenant 1
    assert!(AccessControl::evaluate(
        &bindings_u1,
        Permission::TenantManageUsers,
        &Scope::Tenant {
            tenant_id: tenant_one
        }
    )
    .is_ok());

    // User 1 MUST NOT have access to Tenant 2
    assert!(matches!(
        AccessControl::evaluate(
            &bindings_u1,
            Permission::TenantManageUsers,
            &Scope::Tenant {
                tenant_id: tenant_two
            }
        ),
        Err(IdentityError::Unauthorized { .. })
    ));

    // Sessions for Tenant 1
    let s1 = Session::new(user_one, tenant_one, 3600, 300).unwrap();
    let s2 = Session::new(user_two, tenant_two, 3600, 300).unwrap();
    storage.insert_session(&s1).unwrap();
    storage.insert_session(&s2).unwrap();

    // Bulk tenant revocation on Tenant 1
    storage
        .revoke_all_tenant_sessions(tenant_one, "Tenant suspended")
        .unwrap();

    let s1_fetched = storage.get_session(s1.id).unwrap();
    let s2_fetched = storage.get_session(s2.id).unwrap();

    // Session 1 in Tenant 1 is revoked
    assert!(s1_fetched.revoked);
    assert_eq!(
        s1_fetched.revocation_reason.as_deref(),
        Some("Tenant suspended")
    );

    // Session 2 in Tenant 2 remains untouched and active
    assert!(!s2_fetched.revoked);
}

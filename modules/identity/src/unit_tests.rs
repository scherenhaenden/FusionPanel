use chrono::{Duration, Utc};

use crate::capabilities::{
    get_owned_capabilities, CapabilityStatus, SsoService, TotpService, WebAuthnService,
};
use crate::error::IdentityError;
use crate::password::PasswordManager;
use crate::rbac::{AccessControl, Permission, Role, RoleBinding, Scope};
use crate::session::{RevocationManager, Session};
use crate::types::{SiteId, TenantId, UserId};

#[test]
fn test_password_hashing_and_verification() {
    let password = "SuperSecretPassword123!";
    let hash = PasswordManager::hash_password(password).expect("Hashing should succeed");
    assert!(hash.starts_with("$argon2id$"));

    // Valid verification
    assert!(PasswordManager::verify_password(password, &hash).is_ok());

    // Incorrect password
    let wrong_result = PasswordManager::verify_password("WrongPassword123!", &hash);
    assert_eq!(wrong_result, Err(IdentityError::InvalidCredentials));

    // Empty password edge cases
    assert!(PasswordManager::hash_password("").is_err());
    assert_eq!(
        PasswordManager::verify_password("", &hash),
        Err(IdentityError::InvalidCredentials)
    );

    // Password too short (< 8 chars)
    assert!(PasswordManager::hash_password("short").is_err());
}

#[test]
fn test_rbac_permission_matrix_and_scopes() {
    let user1 = UserId::new_v4();
    let user2 = UserId::new_v4();

    let tenant_a = TenantId::new_v4();
    let tenant_b = TenantId::new_v4();

    let site_a1 = SiteId::new_v4();
    let site_a2 = SiteId::new_v4();
    let site_b1 = SiteId::new_v4();

    let tenant_admin_role = Role::tenant_admin();
    let site_operator_role = Role::site_operator();

    let bindings = [
        // user1 is TenantAdmin for Tenant A
        RoleBinding {
            user_id: user1,
            role: tenant_admin_role,
            scope: Scope::Tenant {
                tenant_id: tenant_a,
            },
        },
        // user2 is SiteOperator for site_a1 only
        RoleBinding {
            user_id: user2,
            role: site_operator_role,
            scope: Scope::Site {
                tenant_id: tenant_a,
                site_id: site_a1,
            },
        },
    ];

    let scope_tenant_a = Scope::Tenant {
        tenant_id: tenant_a,
    };
    let scope_site_a1 = Scope::Site {
        tenant_id: tenant_a,
        site_id: site_a1,
    };
    let scope_site_a2 = Scope::Site {
        tenant_id: tenant_a,
        site_id: site_a2,
    };
    let scope_tenant_b = Scope::Tenant {
        tenant_id: tenant_b,
    };
    let scope_site_b1 = Scope::Site {
        tenant_id: tenant_b,
        site_id: site_b1,
    };

    let user1_bindings = &bindings[0..1];
    let user2_bindings = &bindings[1..2];

    // 1. User1 has TenantAdmin on Tenant A -> can deploy on any site in Tenant A
    assert!(
        AccessControl::evaluate(user1_bindings, Permission::SiteDeploy, &scope_site_a1).is_ok()
    );
    assert!(
        AccessControl::evaluate(user1_bindings, Permission::SiteDeploy, &scope_site_a2).is_ok()
    );
    assert!(AccessControl::evaluate(
        user1_bindings,
        Permission::TenantManageUsers,
        &scope_tenant_a
    )
    .is_ok());

    // Negative: User1 cannot manage Tenant B (Cross-tenant denial)
    assert!(matches!(
        AccessControl::evaluate(user1_bindings, Permission::SiteDeploy, &scope_site_b1),
        Err(IdentityError::Unauthorized { .. })
    ));
    assert!(matches!(
        AccessControl::evaluate(user1_bindings, Permission::TenantRead, &scope_tenant_b),
        Err(IdentityError::Unauthorized { .. })
    ));

    // Negative: User1 does not have SystemAdmin
    assert!(matches!(
        AccessControl::evaluate(user1_bindings, Permission::SystemAdmin, &Scope::Global),
        Err(IdentityError::Unauthorized { .. })
    ));

    // 2. User2 has SiteOperator on site_a1 only
    assert!(
        AccessControl::evaluate(user2_bindings, Permission::SiteDeploy, &scope_site_a1).is_ok()
    );

    // Negative: User2 cannot deploy on site_a2
    assert!(matches!(
        AccessControl::evaluate(user2_bindings, Permission::SiteDeploy, &scope_site_a2),
        Err(IdentityError::Unauthorized { .. })
    ));

    // Negative: User2 cannot delete site or manage users
    assert!(matches!(
        AccessControl::evaluate(user2_bindings, Permission::SiteDelete, &scope_site_a1),
        Err(IdentityError::Unauthorized { .. })
    ));
    assert!(matches!(
        AccessControl::evaluate(
            user2_bindings,
            Permission::TenantManageUsers,
            &scope_tenant_a
        ),
        Err(IdentityError::Unauthorized { .. })
    ));
}

#[test]
fn test_super_admin_global_scope() {
    let super_admin = Role::super_admin();
    let user = UserId::new_v4();
    let bindings = [RoleBinding {
        user_id: user,
        role: super_admin,
        scope: Scope::Global,
    }];

    let any_tenant = Scope::Tenant {
        tenant_id: TenantId::new_v4(),
    };
    let any_site = Scope::Site {
        tenant_id: TenantId::new_v4(),
        site_id: SiteId::new_v4(),
    };

    // SuperAdmin with Global scope can perform any operation anywhere
    assert!(AccessControl::evaluate(&bindings, Permission::SystemAdmin, &Scope::Global).is_ok());
    assert!(AccessControl::evaluate(&bindings, Permission::TenantCreate, &any_tenant).is_ok());
    assert!(AccessControl::evaluate(&bindings, Permission::SiteDelete, &any_site).is_ok());
}

#[test]
fn test_session_expiry_and_idle_timeout() {
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();

    // 3600s TTL, 300s idle timeout
    let mut session = Session::new(user_id, tenant_id, 3600, 300).expect("Session creation failed");
    let now = Utc::now();

    // Active access within idle timeout touches session
    assert!(session
        .validate_and_touch(now + Duration::seconds(100))
        .is_ok());
    assert_eq!(session.last_accessed_at, now + Duration::seconds(100));

    // Exceeded idle timeout (> 300s since last touch)
    assert_eq!(
        session.validate_and_touch(now + Duration::seconds(450)),
        Err(IdentityError::SessionExpired)
    );

    // Fresh session exceeding TTL
    let mut session_ttl = Session::new(user_id, tenant_id, 100, 300).unwrap();
    assert_eq!(
        session_ttl.validate_and_touch(now + Duration::seconds(150)),
        Err(IdentityError::SessionExpired)
    );
}

#[test]
fn test_session_revocation() {
    let user_id = UserId::new_v4();
    let tenant_id = TenantId::new_v4();
    let mut session = Session::new(user_id, tenant_id, 3600, 300).unwrap();
    let now = Utc::now();

    assert!(session.validate_and_touch(now).is_ok());

    // Revoke single session
    session.revoke("User logged out");
    assert!(matches!(
        session.validate_and_touch(now + Duration::seconds(10)),
        Err(IdentityError::SessionRevoked { .. })
    ));

    // Revocation manager test (bulk revocation)
    let mut mgr = RevocationManager::new();
    let mut session2 = Session::new(user_id, tenant_id, 3600, 300).unwrap();
    assert!(mgr.validate_session(&mut session2, now).is_ok());

    // Revoke user sessions
    mgr.revoke_user_sessions(user_id);
    assert!(matches!(
        mgr.validate_session(&mut session2, now),
        Err(IdentityError::SessionRevoked { .. })
    ));
}

#[test]
fn test_capability_matrix_and_unsupported_stubs() {
    let caps = get_owned_capabilities();
    assert_eq!(caps.len(), 5);

    let sec04 = caps.iter().find(|c| c.id == "SEC-04").unwrap();
    assert_eq!(sec04.status, CapabilityStatus::Implemented);

    let sec12 = caps.iter().find(|c| c.id == "SEC-12").unwrap();
    assert_eq!(sec12.status, CapabilityStatus::Implemented);

    let sec01 = caps.iter().find(|c| c.id == "SEC-01").unwrap();
    assert_eq!(sec01.status, CapabilityStatus::Blocked);

    // Stubs must explicitly return IdentityError::Unsupported, never silent success
    assert!(matches!(
        TotpService::setup_totp("user1"),
        Err(IdentityError::Unsupported { .. })
    ));
    assert!(matches!(
        WebAuthnService::register_passkey("user1"),
        Err(IdentityError::Unsupported { .. })
    ));
    assert!(matches!(
        SsoService::authenticate_oauth("google"),
        Err(IdentityError::Unsupported { .. })
    ));
}

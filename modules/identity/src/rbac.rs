use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fmt;
use std::str::FromStr;

use crate::error::IdentityError;
use crate::types::{RoleId, SiteId, TenantId, UserId};

/// Granular permissions in FusionPanel
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    // Global administrative
    SystemAdmin,
    SystemAuditRead,

    // Tenant management
    TenantCreate,
    TenantRead,
    TenantUpdate,
    TenantDelete,
    TenantManageUsers,

    // Site management
    SiteCreate,
    SiteRead,
    SiteUpdate,
    SiteDelete,
    SiteDeploy,

    // Database & Backup operations
    DatabaseManage,
    BackupCreate,
    BackupRestore,

    // DNS & Mail
    DnsManage,
    MailManage,

    // Security & Session
    SessionRevoke,
    UserManage,
}

impl fmt::Display for Permission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Self::SystemAdmin => "system:admin",
            Self::SystemAuditRead => "system:audit_read",
            Self::TenantCreate => "tenant:create",
            Self::TenantRead => "tenant:read",
            Self::TenantUpdate => "tenant:update",
            Self::TenantDelete => "tenant:delete",
            Self::TenantManageUsers => "tenant:manage_users",
            Self::SiteCreate => "site:create",
            Self::SiteRead => "site:read",
            Self::SiteUpdate => "site:update",
            Self::SiteDelete => "site:delete",
            Self::SiteDeploy => "site:deploy",
            Self::DatabaseManage => "database:manage",
            Self::BackupCreate => "backup:create",
            Self::BackupRestore => "backup:restore",
            Self::DnsManage => "dns:manage",
            Self::MailManage => "mail:manage",
            Self::SessionRevoke => "session:revoke",
            Self::UserManage => "user:manage",
        };
        write!(f, "{}", s)
    }
}

impl FromStr for Permission {
    type Err = IdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system:admin" => Ok(Self::SystemAdmin),
            "system:audit_read" => Ok(Self::SystemAuditRead),
            "tenant:create" => Ok(Self::TenantCreate),
            "tenant:read" => Ok(Self::TenantRead),
            "tenant:update" => Ok(Self::TenantUpdate),
            "tenant:delete" => Ok(Self::TenantDelete),
            "tenant:manage_users" => Ok(Self::TenantManageUsers),
            "site:create" => Ok(Self::SiteCreate),
            "site:read" => Ok(Self::SiteRead),
            "site:update" => Ok(Self::SiteUpdate),
            "site:delete" => Ok(Self::SiteDelete),
            "site:deploy" => Ok(Self::SiteDeploy),
            "database:manage" => Ok(Self::DatabaseManage),
            "backup:create" => Ok(Self::BackupCreate),
            "backup:restore" => Ok(Self::BackupRestore),
            "dns:manage" => Ok(Self::DnsManage),
            "mail:manage" => Ok(Self::MailManage),
            "session:revoke" => Ok(Self::SessionRevoke),
            "user:manage" => Ok(Self::UserManage),
            _ => Err(IdentityError::InvalidInput {
                message: format!("Unknown permission: {}", s),
            }),
        }
    }
}

/// Scope of a role or permission assignment
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "type")]
pub enum Scope {
    Global,
    Tenant {
        tenant_id: TenantId,
    },
    Site {
        tenant_id: TenantId,
        site_id: SiteId,
    },
}

impl Scope {
    pub fn matches(&self, requested: &Scope) -> bool {
        match (self, requested) {
            (Scope::Global, _) => true,
            (Scope::Tenant { tenant_id: t1 }, Scope::Tenant { tenant_id: t2 }) => t1 == t2,
            (Scope::Tenant { tenant_id: t1 }, Scope::Site { tenant_id: t2, .. }) => t1 == t2,
            (
                Scope::Site {
                    tenant_id: t1,
                    site_id: s1,
                },
                Scope::Site {
                    tenant_id: t2,
                    site_id: s2,
                },
            ) => t1 == t2 && s1 == s2,
            _ => false,
        }
    }
}

/// Predefined standard role templates
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoleType {
    SuperAdmin,
    TenantAdmin,
    SiteOperator,
    Viewer,
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    pub id: RoleId,
    pub name: String,
    pub role_type: RoleType,
    pub permissions: HashSet<Permission>,
}

impl Role {
    pub fn super_admin() -> Self {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::SystemAdmin);
        permissions.insert(Permission::SystemAuditRead);
        permissions.insert(Permission::TenantCreate);
        permissions.insert(Permission::TenantRead);
        permissions.insert(Permission::TenantUpdate);
        permissions.insert(Permission::TenantDelete);
        permissions.insert(Permission::TenantManageUsers);
        permissions.insert(Permission::SiteCreate);
        permissions.insert(Permission::SiteRead);
        permissions.insert(Permission::SiteUpdate);
        permissions.insert(Permission::SiteDelete);
        permissions.insert(Permission::SiteDeploy);
        permissions.insert(Permission::DatabaseManage);
        permissions.insert(Permission::BackupCreate);
        permissions.insert(Permission::BackupRestore);
        permissions.insert(Permission::DnsManage);
        permissions.insert(Permission::MailManage);
        permissions.insert(Permission::SessionRevoke);
        permissions.insert(Permission::UserManage);

        Self {
            id: RoleId::new_v4(),
            name: "SuperAdmin".to_string(),
            role_type: RoleType::SuperAdmin,
            permissions,
        }
    }

    pub fn tenant_admin() -> Self {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::TenantRead);
        permissions.insert(Permission::TenantUpdate);
        permissions.insert(Permission::TenantManageUsers);
        permissions.insert(Permission::SiteCreate);
        permissions.insert(Permission::SiteRead);
        permissions.insert(Permission::SiteUpdate);
        permissions.insert(Permission::SiteDelete);
        permissions.insert(Permission::SiteDeploy);
        permissions.insert(Permission::DatabaseManage);
        permissions.insert(Permission::BackupCreate);
        permissions.insert(Permission::BackupRestore);
        permissions.insert(Permission::DnsManage);
        permissions.insert(Permission::MailManage);
        permissions.insert(Permission::SessionRevoke);
        permissions.insert(Permission::UserManage);

        Self {
            id: RoleId::new_v4(),
            name: "TenantAdmin".to_string(),
            role_type: RoleType::TenantAdmin,
            permissions,
        }
    }

    pub fn site_operator() -> Self {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::TenantRead);
        permissions.insert(Permission::SiteRead);
        permissions.insert(Permission::SiteUpdate);
        permissions.insert(Permission::SiteDeploy);
        permissions.insert(Permission::BackupCreate);
        permissions.insert(Permission::BackupRestore);

        Self {
            id: RoleId::new_v4(),
            name: "SiteOperator".to_string(),
            role_type: RoleType::SiteOperator,
            permissions,
        }
    }

    pub fn viewer() -> Self {
        let mut permissions = HashSet::new();
        permissions.insert(Permission::TenantRead);
        permissions.insert(Permission::SiteRead);

        Self {
            id: RoleId::new_v4(),
            name: "Viewer".to_string(),
            role_type: RoleType::Viewer,
            permissions,
        }
    }
}

/// Role binding connecting a user, a role, and a scope
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleBinding {
    pub user_id: UserId,
    pub role: Role,
    pub scope: Scope,
}

/// Authorization evaluator enforcing deny-by-default rules and principal filtering
pub struct AccessControl;

impl AccessControl {
    /// Evaluates if the set of bindings allows `permission` on `requested_scope` for `principal_id`.
    /// Deny by default: returns Ok(()) only if at least one matching binding belongs to the principal
    /// and grants the permission on the target scope.
    pub fn evaluate_principal(
        principal_id: UserId,
        bindings: &[RoleBinding],
        permission: Permission,
        requested_scope: &Scope,
    ) -> Result<(), IdentityError> {
        for binding in bindings {
            if binding.user_id == principal_id
                && binding.scope.matches(requested_scope)
                && binding.role.permissions.contains(&permission)
            {
                return Ok(());
            }
        }

        Err(IdentityError::Unauthorized {
            reason: format!(
                "Principal '{}' denied permission '{}' on scope {:?}",
                principal_id, permission, requested_scope
            ),
        })
    }

    /// Evaluates if ANY supplied binding allows `permission` on `requested_scope`.
    pub fn evaluate(
        bindings: &[RoleBinding],
        permission: Permission,
        requested_scope: &Scope,
    ) -> Result<(), IdentityError> {
        for binding in bindings {
            if binding.scope.matches(requested_scope)
                && binding.role.permissions.contains(&permission)
            {
                return Ok(());
            }
        }

        Err(IdentityError::Unauthorized {
            reason: format!(
                "Permission '{}' denied on target scope {:?}",
                permission, requested_scope
            ),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_principal_scoping_enforcement() {
        let principal1 = UserId::new_v4();
        let principal2 = UserId::new_v4();
        let tenant = TenantId::new_v4();

        let binding_p1 = RoleBinding {
            user_id: principal1,
            role: Role::tenant_admin(),
            scope: Scope::Tenant { tenant_id: tenant },
        };

        let bindings = vec![binding_p1];

        // Principal 1 is allowed
        let allowed = AccessControl::evaluate_principal(
            principal1,
            &bindings,
            Permission::SiteCreate,
            &Scope::Tenant { tenant_id: tenant },
        );
        assert!(allowed.is_ok());

        // Principal 2 must be DENIED even if bindings array contains principal 1's role
        let denied = AccessControl::evaluate_principal(
            principal2,
            &bindings,
            Permission::SiteCreate,
            &Scope::Tenant { tenant_id: tenant },
        );
        assert!(matches!(denied, Err(IdentityError::Unauthorized { .. })));
    }

    #[test]
    fn test_tenant_isolation() {
        let user = UserId::new_v4();
        let tenant_a = TenantId::new_v4();
        let tenant_b = TenantId::new_v4();

        let binding = RoleBinding {
            user_id: user,
            role: Role::tenant_admin(),
            scope: Scope::Tenant {
                tenant_id: tenant_a,
            },
        };

        let bindings = vec![binding];

        // Granted on Tenant A
        assert!(AccessControl::evaluate_principal(
            user,
            &bindings,
            Permission::SiteCreate,
            &Scope::Tenant {
                tenant_id: tenant_a
            }
        )
        .is_ok());

        // Denied on Tenant B (Strict Tenant Isolation)
        let cross_tenant = AccessControl::evaluate_principal(
            user,
            &bindings,
            Permission::SiteCreate,
            &Scope::Tenant {
                tenant_id: tenant_b,
            },
        );
        assert!(matches!(
            cross_tenant,
            Err(IdentityError::Unauthorized { .. })
        ));
    }

    #[test]
    fn test_site_scope_containment() {
        let user = UserId::new_v4();
        let tenant = TenantId::new_v4();
        let site_1 = SiteId::new_v4();
        let site_2 = SiteId::new_v4();

        let binding = RoleBinding {
            user_id: user,
            role: Role::site_operator(),
            scope: Scope::Site {
                tenant_id: tenant,
                site_id: site_1,
            },
        };

        let bindings = vec![binding];

        // Operator has permission on Site 1
        assert!(AccessControl::evaluate_principal(
            user,
            &bindings,
            Permission::SiteDeploy,
            &Scope::Site {
                tenant_id: tenant,
                site_id: site_1
            }
        )
        .is_ok());

        // Operator CANNOT deploy Site 2
        let site2_denied = AccessControl::evaluate_principal(
            user,
            &bindings,
            Permission::SiteDeploy,
            &Scope::Site {
                tenant_id: tenant,
                site_id: site_2,
            },
        );
        assert!(matches!(
            site2_denied,
            Err(IdentityError::Unauthorized { .. })
        ));

        // Operator CANNOT perform tenant-level operations
        let tenant_denied = AccessControl::evaluate_principal(
            user,
            &bindings,
            Permission::SiteDeploy,
            &Scope::Tenant { tenant_id: tenant },
        );
        assert!(matches!(
            tenant_denied,
            Err(IdentityError::Unauthorized { .. })
        ));
    }

    #[test]
    fn test_super_admin_global_access() {
        let admin = UserId::new_v4();
        let tenant = TenantId::new_v4();
        let site = SiteId::new_v4();

        let binding = RoleBinding {
            user_id: admin,
            role: Role::super_admin(),
            scope: Scope::Global,
        };

        let bindings = vec![binding];

        // Global super admin has permission anywhere
        assert!(AccessControl::evaluate_principal(
            admin,
            &bindings,
            Permission::SystemAdmin,
            &Scope::Global
        )
        .is_ok());
        assert!(AccessControl::evaluate_principal(
            admin,
            &bindings,
            Permission::SiteDelete,
            &Scope::Tenant { tenant_id: tenant }
        )
        .is_ok());
        assert!(AccessControl::evaluate_principal(
            admin,
            &bindings,
            Permission::SiteDeploy,
            &Scope::Site {
                tenant_id: tenant,
                site_id: site
            }
        )
        .is_ok());
    }

    #[test]
    fn test_deny_by_default_and_role_demotion() {
        let user = UserId::new_v4();
        let tenant = TenantId::new_v4();

        // Viewer role does NOT have SiteCreate permission
        let binding = RoleBinding {
            user_id: user,
            role: Role::viewer(),
            scope: Scope::Tenant { tenant_id: tenant },
        };

        let mut bindings = vec![binding];

        // Viewer can read site
        assert!(AccessControl::evaluate_principal(
            user,
            &bindings,
            Permission::SiteRead,
            &Scope::Tenant { tenant_id: tenant }
        )
        .is_ok());

        // Viewer cannot create site
        assert!(matches!(
            AccessControl::evaluate_principal(
                user,
                &bindings,
                Permission::SiteCreate,
                &Scope::Tenant { tenant_id: tenant }
            ),
            Err(IdentityError::Unauthorized { .. })
        ));

        // Demotion/Revocation: Remove all bindings
        bindings.clear();
        assert!(matches!(
            AccessControl::evaluate_principal(
                user,
                &bindings,
                Permission::SiteRead,
                &Scope::Tenant { tenant_id: tenant }
            ),
            Err(IdentityError::Unauthorized { .. })
        ));
    }
}

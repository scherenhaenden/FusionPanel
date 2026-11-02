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

/// Authorization evaluator enforcing deny-by-default rules
pub struct AccessControl;

impl AccessControl {
    /// Evaluates if the set of bindings allows `permission` on `requested_scope`.
    /// Deny by default: returns Ok(()) only if at least one matching binding includes the permission,
    /// otherwise Err(IdentityError::Unauthorized).
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

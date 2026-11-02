#![forbid(unsafe_code)]

//! Identity and permissions module for FusionPanel (S08).

pub mod capabilities;
pub mod error;
pub mod password;
pub mod rbac;
pub mod session;
pub mod storage;
pub mod types;

#[cfg(test)]
mod unit_tests;

pub use capabilities::{get_owned_capabilities, CapabilityInfo, CapabilityStatus};
pub use error::IdentityError;
pub use password::PasswordManager;
pub use rbac::{AccessControl, Permission, Role, RoleBinding, RoleType, Scope};
pub use session::{RevocationManager, Session};
pub use storage::IdentityStorage;
pub use types::{
    AppliedResult, ObservedState, Plan, RoleId, SessionId, SiteId, TenantId, User, UserId,
    UserStatus,
};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

use crate::error::IdentityError;

macro_rules! define_id {
    ($name:ident, $desc:literal) => {
        #[derive(
            Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new_v4() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn as_uuid(&self) -> &Uuid {
                &self.0
            }

            pub fn to_uuid(self) -> Uuid {
                self.0
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = IdentityError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s)
                    .map($name)
                    .map_err(|_| IdentityError::InvalidInput {
                        message: format!("Invalid {} UUID: {}", $desc, s),
                    })
            }
        }

        impl From<Uuid> for $name {
            fn from(u: Uuid) -> Self {
                Self(u)
            }
        }
    };
}

define_id!(UserId, "user ID");
define_id!(TenantId, "tenant ID");
define_id!(SiteId, "site ID");
define_id!(SessionId, "session ID");
define_id!(RoleId, "role ID");

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UserStatus {
    Active,
    Suspended,
    Revoked,
}

impl fmt::Display for UserStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Suspended => write!(f, "suspended"),
            Self::Revoked => write!(f, "revoked"),
        }
    }
}

impl FromStr for UserStatus {
    type Err = IdentityError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "suspended" => Ok(Self::Suspended),
            "revoked" => Ok(Self::Revoked),
            _ => Err(IdentityError::InvalidInput {
                message: format!("Unknown user status: {}", s),
            }),
        }
    }
}

/// User domain representation.
/// Notice that raw credentials or secret material are never stored in plain text or logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    pub id: UserId,
    pub username: String,
    pub email: String,
    pub status: UserStatus,
    pub password_hash: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl User {
    pub fn validate(&self) -> Result<(), IdentityError> {
        if self.username.trim().is_empty() {
            return Err(IdentityError::InvalidInput {
                message: "Username cannot be empty".to_string(),
            });
        }
        if self.email.trim().is_empty() || !self.email.contains('@') {
            return Err(IdentityError::InvalidInput {
                message: "Invalid email address format".to_string(),
            });
        }
        if self.password_hash.trim().is_empty() {
            return Err(IdentityError::InvalidInput {
                message: "Password hash cannot be empty".to_string(),
            });
        }
        Ok(())
    }
}

/// Operation plan contract following docs/engineering/MODULE_CONTRACTS.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plan<T> {
    pub plan_id: Uuid,
    pub operation: String,
    pub payload: T,
    pub prerequisites: Vec<String>,
    pub required_permissions: Vec<String>,
    pub created_at: DateTime<Utc>,
}

/// Operation applied result contract following docs/engineering/MODULE_CONTRACTS.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AppliedResult<R> {
    pub plan_id: Uuid,
    pub status: String,
    pub resource_id: String,
    pub details: R,
    pub applied_at: DateTime<Utc>,
}

/// Observed state contract following docs/engineering/MODULE_CONTRACTS.md
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedState<S> {
    pub resource_id: String,
    pub state: S,
    pub observed_at: DateTime<Utc>,
}

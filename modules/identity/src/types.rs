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
/// Secret material and plaintext passwords are never stored or logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "UserRaw")]
pub struct User {
    id: UserId,
    username: String,
    email: String,
    status: UserStatus,
    password_hash: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

#[derive(Deserialize)]
struct UserRaw {
    id: UserId,
    username: String,
    email: String,
    status: UserStatus,
    password_hash: String,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl TryFrom<UserRaw> for User {
    type Error = String;

    fn try_from(raw: UserRaw) -> Result<Self, Self::Error> {
        let user = Self {
            id: raw.id,
            username: raw.username,
            email: raw.email,
            status: raw.status,
            password_hash: raw.password_hash,
            created_at: raw.created_at,
            updated_at: raw.updated_at,
        };
        user.validate().map_err(|e| e.to_string())?;
        Ok(user)
    }
}

impl User {
    pub fn new(
        username: String,
        email: String,
        password_hash: String,
    ) -> Result<Self, IdentityError> {
        let now = Utc::now();
        let user = Self {
            id: UserId::new_v4(),
            username,
            email,
            status: UserStatus::Active,
            password_hash,
            created_at: now,
            updated_at: now,
        };
        user.validate()?;
        Ok(user)
    }

    pub fn from_persisted(
        id: UserId,
        username: String,
        email: String,
        status: UserStatus,
        password_hash: String,
        created_at: DateTime<Utc>,
        updated_at: DateTime<Utc>,
    ) -> Result<Self, IdentityError> {
        let user = Self {
            id,
            username,
            email,
            status,
            password_hash,
            created_at,
            updated_at,
        };
        user.validate()?;
        Ok(user)
    }

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

    pub fn id(&self) -> UserId {
        self.id
    }

    pub fn username(&self) -> &str {
        &self.username
    }

    pub fn email(&self) -> &str {
        &self.email
    }

    pub fn status(&self) -> UserStatus {
        self.status
    }

    pub fn password_hash(&self) -> &str {
        &self.password_hash
    }

    pub fn created_at(&self) -> DateTime<Utc> {
        self.created_at
    }

    pub fn updated_at(&self) -> DateTime<Utc> {
        self.updated_at
    }

    pub fn set_status(&mut self, status: UserStatus) {
        self.status = status;
        self.updated_at = Utc::now();
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
pub struct ObservedState<T> {
    pub resource_id: String,
    pub state: T,
    pub observed_at: DateTime<Utc>,
}

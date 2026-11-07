#![forbid(unsafe_code)]

use serde::{Deserialize, Serialize};
use std::fmt;
use std::str::FromStr;
use thiserror::Error;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                write!(f, "{}", self.0)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Ok(Self(Uuid::parse_str(s)?))
            }
        }
    };
}

define_id!(NodeId);
define_id!(SiteId);
define_id!(TenantId);
define_id!(JobId);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ResourceOwner {
    pub tenant_id: TenantId,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteKind {
    Php,
    Static,
    ReverseProxy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResourceState {
    Pending,
    Active,
    Suspended,
    Failed,
    Deleting,
    Deleted,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("Invalid resource state transition from {from:?} to {to:?}")]
pub struct StateTransitionError {
    pub from: ResourceState,
    pub to: ResourceState,
}

impl ResourceState {
    pub fn transition_to(self, to: ResourceState) -> Result<ResourceState, StateTransitionError> {
        let valid = matches!(
            (self, to),
            (Self::Pending, Self::Active)
                | (Self::Pending, Self::Failed)
                | (Self::Pending, Self::Deleting)
                | (Self::Active, Self::Suspended)
                | (Self::Active, Self::Deleting)
                | (Self::Suspended, Self::Active)
                | (Self::Suspended, Self::Deleting)
                | (Self::Failed, Self::Deleting)
                | (Self::Deleting, Self::Deleted)
                | (Self::Deleting, Self::Failed)
        );

        if valid {
            Ok(to)
        } else {
            Err(StateTransitionError { from: self, to })
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DomainName(String);

impl fmt::Display for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<DomainName> for String {
    fn from(d: DomainName) -> Self {
        d.0
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainNameError {
    #[error("Domain name is empty")]
    Empty,
    #[error("Domain name exceeds 253 characters")]
    TooLong,
    #[error("Domain name contains non-ASCII characters (IDNA unsupported)")]
    NonAscii,
    #[error("Domain name is an IP literal")]
    IpLiteral,
    #[error("Domain label is empty")]
    EmptyLabel,
    #[error("Domain label exceeds 63 characters")]
    LabelTooLong,
    #[error("Domain label contains invalid characters")]
    InvalidCharacters,
    #[error("Domain label starts or ends with a hyphen")]
    InvalidHyphen,
}

impl TryFrom<String> for DomainName {
    type Error = DomainNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        DomainName::from_str(&value)
    }
}

impl FromStr for DomainName {
    type Err = DomainNameError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s.is_empty() {
            return Err(DomainNameError::Empty);
        }
        if s.len() > 253 {
            return Err(DomainNameError::TooLong);
        }
        if !s.is_ascii() {
            return Err(DomainNameError::NonAscii);
        }
        if s.parse::<std::net::IpAddr>().is_ok() {
            return Err(DomainNameError::IpLiteral);
        }

        for label in s.split('.') {
            if label.is_empty() {
                return Err(DomainNameError::EmptyLabel);
            }
            if label.len() > 63 {
                return Err(DomainNameError::LabelTooLong);
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainNameError::InvalidHyphen);
            }
            for b in label.bytes() {
                if !b.is_ascii_alphanumeric() && b != b'-' {
                    return Err(DomainNameError::InvalidCharacters);
                }
            }
        }

        Ok(Self(s.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_domain_name_validation() {
        assert!(DomainName::from_str("example.com").is_ok());
        assert!(DomainName::from_str("127.0.0.1").is_err());
        assert!(DomainName::from_str("éxample.com").is_err());
        assert!(DomainName::from_str("-example.com").is_err());
        assert!(DomainName::from_str("example.com-").is_err());
    }

    #[test]
    fn test_resource_state_transition() {
        assert!(ResourceState::Pending
            .transition_to(ResourceState::Active)
            .is_ok());
        assert!(ResourceState::Active
            .transition_to(ResourceState::Pending)
            .is_err());
    }
}

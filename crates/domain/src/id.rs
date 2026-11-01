use std::fmt;
use std::str::FromStr;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use crate::error::DomainError;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            pub fn as_uuid(&self) -> &Uuid {
                &self.0
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
            type Err = DomainError;

            fn from_str(s: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(s)
                    .map(Self)
                    .map_err(|e| DomainError::InvalidId(e.to_string()))
            }
        }

        impl From<Uuid> for $name {
            fn from(uuid: Uuid) -> Self {
                Self(uuid)
            }
        }

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: serde::Serializer,
            {
                serializer.serialize_str(&self.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: serde::Deserializer<'de>,
            {
                let s = String::deserialize(deserializer)?;
                Self::from_str(&s).map_err(serde::de::Error::custom)
            }
        }
    };
}

define_id!(NodeId);
define_id!(SiteId);
define_id!(TenantId);
define_id!(JobId);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_id_roundtrips() {
        let id = NodeId::new();
        let s = id.to_string();
        let parsed: NodeId = s.parse().unwrap();
        assert_eq!(id, parsed);

        let json = serde_json::to_string(&id).unwrap();
        assert_eq!(json, format!("\"{}\"", id));
        let deserialized: NodeId = serde_json::from_str(&json).unwrap();
        assert_eq!(id, deserialized);
    }

    #[test]
    fn test_invalid_id() {
        let err = NodeId::from_str("not-a-uuid").unwrap_err();
        match err {
            DomainError::InvalidId(_) => (),
            _ => panic!("Expected InvalidId error"),
        }
    }
}

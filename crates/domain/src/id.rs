use crate::error::DomainError;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub struct $name(Uuid);

        impl Serialize for $name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: Serializer,
            {
                serializer.serialize_str(&self.0.to_string())
            }
        }

        impl<'de> Deserialize<'de> for $name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                let value = String::deserialize(deserializer)?;
                Uuid::parse_str(&value).map(Self).map_err(D::Error::custom)
            }
        }

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
    };
}

define_id!(NodeId);
define_id!(SiteId);
define_id!(TenantId);
define_id!(JobId);

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_non_human_readable_string_encoding<T>(value: T)
    where
        T: Serialize + serde::de::DeserializeOwned + fmt::Display + fmt::Debug + PartialEq,
    {
        let string_encoding = bincode::serialize(&value.to_string()).unwrap();
        let id_encoding = bincode::serialize(&value).unwrap();
        assert_eq!(id_encoding, string_encoding);

        let decoded: T = bincode::deserialize(&id_encoding).unwrap();
        assert_eq!(decoded, value);
    }

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

    #[test]
    fn test_ids_use_uuid_strings_in_non_human_readable_formats() {
        assert_non_human_readable_string_encoding(NodeId::new());
        assert_non_human_readable_string_encoding(SiteId::new());
        assert_non_human_readable_string_encoding(TenantId::new());
        assert_non_human_readable_string_encoding(JobId::new());
    }
}

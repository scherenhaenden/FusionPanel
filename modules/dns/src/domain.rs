use serde::{de::Error as _, Deserialize, Deserializer, Serialize};
use std::fmt;
use thiserror::Error;
use uuid::Uuid;

macro_rules! define_uuid_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(f)
            }
        }
    };
}

define_uuid_id!(NodeId);
define_uuid_id!(SiteId);
define_uuid_id!(TenantId);
define_uuid_id!(JobId);

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("Invalid DNS label: {0}")]
    InvalidLabel(String),
    #[error("Label too long (max 63 bytes)")]
    LabelTooLong,
    #[error("Domain name too long (max 253 bytes)")]
    DomainTooLong,
    #[error("IP literal unsupported as domain name")]
    IpLiteralUnsupported,
    #[error("non-ASCII domain names are unsupported; IDNA conversion is not provided")]
    IdnaUnsupported,
    #[error("Invalid TTL value")]
    InvalidTtl,
    #[error("Invalid Record data: {0}")]
    InvalidRecordData(String),
    #[error("record owner `{owner}` is outside zone `{zone}`")]
    RecordOutsideZone { owner: String, zone: String },
    #[error("a Null MX record cannot coexist with another MX record at the same owner")]
    ConflictingNullMx,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize)]
#[serde(transparent)]
pub struct DomainName(String);

impl DomainName {
    pub fn parse(name: &str) -> Result<Self, DomainError> {
        if name.is_empty() || name == "." {
            return Err(DomainError::InvalidLabel("Empty domain".to_string()));
        }

        if !name.is_ascii() {
            return Err(DomainError::IdnaUnsupported);
        }

        let canonical_name = match name.strip_suffix('.') {
            Some(without_root_dot) if without_root_dot.ends_with('.') => {
                return Err(DomainError::InvalidLabel(
                    "Multiple trailing root dots".to_string(),
                ));
            }
            Some(without_root_dot) => without_root_dot,
            None => name,
        };

        if canonical_name.is_empty() {
            return Err(DomainError::InvalidLabel("Empty domain".to_string()));
        }

        if canonical_name.len() > 253 {
            return Err(DomainError::DomainTooLong);
        }

        if canonical_name.parse::<std::net::IpAddr>().is_ok()
            || is_legacy_ipv4_address(canonical_name)
        {
            return Err(DomainError::IpLiteralUnsupported);
        }

        for label in canonical_name.split('.') {
            if label.is_empty() {
                return Err(DomainError::InvalidLabel("Empty label".to_string()));
            }
            if label.len() > 63 {
                return Err(DomainError::LabelTooLong);
            }
            if !label
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '*' )
            {
                return Err(DomainError::InvalidLabel(label.to_string()));
            }
            if label.starts_with('-') || label.ends_with('-') {
                return Err(DomainError::InvalidLabel(
                    "Label cannot start or end with hyphen".to_string(),
                ));
            }
        }

        Ok(Self(canonical_name.to_ascii_lowercase()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl<'de> Deserialize<'de> for DomainName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let input = String::deserialize(deserializer)?;
        Self::parse(&input).map_err(D::Error::custom)
    }
}

fn is_legacy_ipv4_address(input: &str) -> bool {
    let labels: Vec<&str> = input.split('.').collect();
    if labels.is_empty() || labels.len() > 4 {
        return false;
    }

    let Some(numbers) = labels
        .iter()
        .map(|label| parse_legacy_ipv4_number(label))
        .collect::<Option<Vec<_>>>()
    else {
        return false;
    };

    if numbers[..numbers.len() - 1].iter().any(|part| *part > 255) {
        return false;
    }

    let final_part_limit = 1_u64 << (8 * (5 - numbers.len()));
    numbers[numbers.len() - 1] < final_part_limit
}

fn parse_legacy_ipv4_number(input: &str) -> Option<u64> {
    let (digits, radix) = if input.starts_with("0x") || input.starts_with("0X") {
        let digits = &input[2..];
        if digits.is_empty() {
            return Some(0);
        }
        (digits, 16)
    } else if input.len() > 1 && input.starts_with('0') {
        (&input[1..], 8)
    } else {
        (input, 10)
    };

    if digits.is_empty() {
        return Some(0);
    }

    u64::from_str_radix(digits, radix).ok()
}

impl fmt::Display for DomainName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecordType {
    A,
    AAAA,
    CNAME,
    MX,
    TXT,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Record {
    name: DomainName,
    record_type: RecordType,
    ttl: u32,
    data: String,
}

impl Record {
    pub fn new(
        name: DomainName,
        record_type: RecordType,
        ttl: u32,
        data: String,
    ) -> Result<Self, DomainError> {
        if ttl > 2147483647 {
            return Err(DomainError::InvalidTtl);
        }

        // Basic record data validation
        match record_type {
            RecordType::A => {
                data.parse::<std::net::Ipv4Addr>().map_err(|_| {
                    DomainError::InvalidRecordData("Invalid IPv4 address".to_string())
                })?;
            }
            RecordType::AAAA => {
                data.parse::<std::net::Ipv6Addr>().map_err(|_| {
                    DomainError::InvalidRecordData("Invalid IPv6 address".to_string())
                })?;
            }
            RecordType::CNAME => {
                DomainName::parse(&data)?;
            }
            RecordType::MX => {
                let parts: Vec<&str> = data.split_whitespace().collect();
                if parts.len() != 2 {
                    return Err(DomainError::InvalidRecordData(
                        "MX record must have priority and target".to_string(),
                    ));
                }
                let priority = parts[0].parse::<u16>().map_err(|_| {
                    DomainError::InvalidRecordData("Invalid MX priority".to_string())
                })?;
                if parts[1] == "." {
                    if priority != 0 {
                        return Err(DomainError::InvalidRecordData(
                            "Null MX must have priority 0".to_string(),
                        ));
                    }
                } else {
                    DomainName::parse(parts[1])?;
                }
            }
            RecordType::TXT => {
                if data.is_empty() || data.len() > 255 {
                    return Err(DomainError::InvalidRecordData(
                        "TXT record data must be 1-255 characters".to_string(),
                    ));
                }
            }
        }

        Ok(Self {
            name,
            record_type,
            ttl,
            data,
        })
    }

    pub fn name(&self) -> &DomainName {
        &self.name
    }

    pub fn record_type(&self) -> RecordType {
        self.record_type
    }

    pub fn ttl(&self) -> u32 {
        self.ttl
    }

    pub fn data(&self) -> &str {
        &self.data
    }
}

impl<'de> Deserialize<'de> for Record {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct RecordInput {
            name: DomainName,
            record_type: RecordType,
            ttl: u32,
            data: String,
        }

        let input = RecordInput::deserialize(deserializer)?;
        Self::new(input.name, input.record_type, input.ttl, input.data).map_err(D::Error::custom)
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct Zone {
    id: Uuid,
    name: DomainName,
    tenant_id: TenantId,
    records: Vec<Record>,
}

impl Zone {
    pub fn new(name: DomainName, tenant_id: TenantId) -> Self {
        Self {
            id: Uuid::new_v4(),
            name,
            tenant_id,
            records: Vec::new(),
        }
    }

    pub fn add_record(&mut self, record: Record) -> Result<(), DomainError> {
        if !record.name.is_within(&self.name) {
            return Err(DomainError::RecordOutsideZone {
                owner: record.name.to_string(),
                zone: self.name.to_string(),
            });
        }

        if record.record_type == RecordType::MX {
            let is_null_mx = record.data.split_whitespace().nth(1) == Some(".");
            if self.records.iter().any(|existing| {
                existing.name == record.name
                    && existing.record_type == RecordType::MX
                    && (is_null_mx || existing.data.split_whitespace().nth(1) == Some("."))
            }) {
                return Err(DomainError::ConflictingNullMx);
            }
        }

        self.records.push(record);
        Ok(())
    }

    pub fn remove_record(&mut self, record: &Record) -> bool {
        let len_before = self.records.len();
        self.records.retain(|r| r != record);
        self.records.len() < len_before
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn name(&self) -> &DomainName {
        &self.name
    }

    pub fn tenant_id(&self) -> TenantId {
        self.tenant_id
    }

    pub fn records(&self) -> &[Record] {
        &self.records
    }

    fn is_valid_owner(&self, owner: &DomainName) -> bool {
        owner.is_within(&self.name)
    }
}

impl DomainName {
    pub fn is_within(&self, zone: &Self) -> bool {
        self == zone
            || self
                .0
                .strip_suffix(&zone.0)
                .is_some_and(|prefix| prefix.ends_with('.'))
    }
}

impl<'de> Deserialize<'de> for Zone {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct ZoneInput {
            id: Uuid,
            name: DomainName,
            tenant_id: TenantId,
            records: Vec<Record>,
        }

        let input = ZoneInput::deserialize(deserializer)?;
        let mut zone = Self {
            id: input.id,
            name: input.name,
            tenant_id: input.tenant_id,
            records: Vec::with_capacity(input.records.len()),
        };
        for record in input.records {
            if !zone.is_valid_owner(&record.name) {
                return Err(D::Error::custom(DomainError::RecordOutsideZone {
                    owner: record.name.to_string(),
                    zone: zone.name.to_string(),
                }));
            }
            zone.add_record(record).map_err(D::Error::custom)?;
        }
        Ok(zone)
    }
}

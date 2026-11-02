use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TenantId(Uuid);

impl Default for TenantId {
    fn default() -> Self {
        Self::new()
    }
}

impl TenantId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanTier {
    Free,
    Basic,
    Pro,
    Enterprise,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum AccountStatus {
    Pending,
    Active,
    Suspended,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Organization {
    pub id: TenantId,
    pub name: String,
    pub plan: PlanTier,
    pub status: AccountStatus,
}

impl Organization {
    pub fn new(name: impl Into<String>) -> Result<Self, &'static str> {
        let name = name.into();
        if name.trim().is_empty() {
            return Err("Organization name cannot be empty");
        }
        if name.len() > 100 {
            return Err("Organization name too long");
        }
        Ok(Self {
            id: TenantId::new(),
            name,
            plan: PlanTier::Free,
            status: AccountStatus::Pending,
        })
    }
}

#![forbid(unsafe_code)]

use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum QuotaError {
    #[error("Storage quota exceeded: current {current}, requested {requested}, limit {limit}")]
    StorageExceeded {
        current: u64,
        requested: u64,
        limit: u64,
    },
    #[error("Table count quota exceeded: current {current}, requested {requested}, limit {limit}")]
    TableCountExceeded {
        current: u32,
        requested: u32,
        limit: u32,
    },
    #[error("Tenant {0} not found")]
    TenantNotFound(Uuid),
}

#[derive(Debug, Clone, Default)]
pub struct QuotaLimits {
    pub max_storage_bytes: u64,
    pub max_table_count: u32,
}

#[derive(Debug, Clone, Default)]
pub struct QuotaUsage {
    pub storage_bytes: u64,
    pub table_count: u32,
}

#[derive(Debug, Clone)]
pub struct QuotaPercentage {
    pub storage_percentage: f64,
    pub table_count_percentage: f64,
}

pub struct TenantQuota {
    pub limits: QuotaLimits,
    pub usage: QuotaUsage,
}

pub struct QuotaEnforcer {
    tenants: HashMap<Uuid, TenantQuota>,
}

impl Default for QuotaEnforcer {
    fn default() -> Self {
        Self::new()
    }
}

impl QuotaEnforcer {
    pub fn new() -> Self {
        Self {
            tenants: HashMap::new(),
        }
    }

    pub fn set_limits(&mut self, tenant_id: Uuid, limits: QuotaLimits) {
        let tenant = self.tenants.entry(tenant_id).or_insert_with(|| TenantQuota {
            limits: QuotaLimits::default(),
            usage: QuotaUsage::default(),
        });
        tenant.limits = limits;
    }

    pub fn set_usage(&mut self, tenant_id: Uuid, usage: QuotaUsage) {
        let tenant = self.tenants.entry(tenant_id).or_insert_with(|| TenantQuota {
            limits: QuotaLimits::default(),
            usage: QuotaUsage::default(),
        });
        tenant.usage = usage;
    }

    pub fn get_usage(&self, tenant_id: Uuid) -> Result<QuotaUsage, QuotaError> {
        self.tenants
            .get(&tenant_id)
            .map(|t| t.usage.clone())
            .ok_or(QuotaError::TenantNotFound(tenant_id))
    }

    pub fn get_limits(&self, tenant_id: Uuid) -> Result<QuotaLimits, QuotaError> {
        self.tenants
            .get(&tenant_id)
            .map(|t| t.limits.clone())
            .ok_or(QuotaError::TenantNotFound(tenant_id))
    }

    pub fn check_allocation(
        &self,
        tenant_id: Uuid,
        additional_storage_bytes: u64,
        additional_table_count: u32,
    ) -> Result<(), QuotaError> {
        let tenant = self
            .tenants
            .get(&tenant_id)
            .ok_or(QuotaError::TenantNotFound(tenant_id))?;

        if tenant.usage.storage_bytes + additional_storage_bytes > tenant.limits.max_storage_bytes {
            return Err(QuotaError::StorageExceeded {
                current: tenant.usage.storage_bytes,
                requested: additional_storage_bytes,
                limit: tenant.limits.max_storage_bytes,
            });
        }

        if tenant.usage.table_count + additional_table_count > tenant.limits.max_table_count {
            return Err(QuotaError::TableCountExceeded {
                current: tenant.usage.table_count,
                requested: additional_table_count,
                limit: tenant.limits.max_table_count,
            });
        }

        Ok(())
    }

    pub fn allocate(
        &mut self,
        tenant_id: Uuid,
        additional_storage_bytes: u64,
        additional_table_count: u32,
    ) -> Result<(), QuotaError> {
        self.check_allocation(tenant_id, additional_storage_bytes, additional_table_count)?;

        if let Some(tenant) = self.tenants.get_mut(&tenant_id) {
            tenant.usage.storage_bytes += additional_storage_bytes;
            tenant.usage.table_count += additional_table_count;
        }

        Ok(())
    }

    pub fn release(
        &mut self,
        tenant_id: Uuid,
        released_storage_bytes: u64,
        released_table_count: u32,
    ) -> Result<(), QuotaError> {
        let tenant = self
            .tenants
            .get_mut(&tenant_id)
            .ok_or(QuotaError::TenantNotFound(tenant_id))?;

        tenant.usage.storage_bytes = tenant
            .usage
            .storage_bytes
            .saturating_sub(released_storage_bytes);
        tenant.usage.table_count = tenant
            .usage
            .table_count
            .saturating_sub(released_table_count);

        Ok(())
    }

    pub fn calculate_usage_percentages(
        &self,
        tenant_id: Uuid,
    ) -> Result<QuotaPercentage, QuotaError> {
        let tenant = self
            .tenants
            .get(&tenant_id)
            .ok_or(QuotaError::TenantNotFound(tenant_id))?;

        let storage_percentage = if tenant.limits.max_storage_bytes == 0 {
            if tenant.usage.storage_bytes > 0 {
                100.0
            } else {
                0.0
            }
        } else {
            (tenant.usage.storage_bytes as f64 / tenant.limits.max_storage_bytes as f64) * 100.0
        };

        let table_count_percentage = if tenant.limits.max_table_count == 0 {
            if tenant.usage.table_count > 0 {
                100.0
            } else {
                0.0
            }
        } else {
            (tenant.usage.table_count as f64 / tenant.limits.max_table_count as f64) * 100.0
        };

        Ok(QuotaPercentage {
            storage_percentage,
            table_count_percentage,
        })
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResourceQuotas {
    pub cpu_cores: u32,
    pub memory_mb: u64,
    pub disk_mb: u64,
    pub domain_count: u32,
    pub database_count: u32,
}

impl ResourceQuotas {
    pub fn add(&mut self, other: &ResourceQuotas) {
        self.cpu_cores += other.cpu_cores;
        self.memory_mb += other.memory_mb;
        self.disk_mb += other.disk_mb;
        self.domain_count += other.domain_count;
        self.database_count += other.database_count;
    }

    pub fn sub(&mut self, other: &ResourceQuotas) {
        self.cpu_cores = self.cpu_cores.saturating_sub(other.cpu_cores);
        self.memory_mb = self.memory_mb.saturating_sub(other.memory_mb);
        self.disk_mb = self.disk_mb.saturating_sub(other.disk_mb);
        self.domain_count = self.domain_count.saturating_sub(other.domain_count);
        self.database_count = self.database_count.saturating_sub(other.database_count);
    }

    pub fn fits_in(&self, limit: &ResourceQuotas) -> bool {
        self.cpu_cores <= limit.cpu_cores
            && self.memory_mb <= limit.memory_mb
            && self.disk_mb <= limit.disk_mb
            && self.domain_count <= limit.domain_count
            && self.database_count <= limit.database_count
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QuotaError {
    #[error("Quota limit exceeded for one or more resources")]
    LimitExceeded,
    #[error("Tenant limits not configured")]
    LimitsNotConfigured,
}

pub struct QuotaEngine {
    state: Arc<Mutex<QuotaState>>,
}

struct QuotaState {
    limits: HashMap<Uuid, ResourceQuotas>,
    usage: HashMap<Uuid, ResourceQuotas>,
}

impl QuotaEngine {
    pub fn new() -> Self {
        Self {
            state: Arc::new(Mutex::new(QuotaState {
                limits: HashMap::new(),
                usage: HashMap::new(),
            })),
        }
    }

    pub fn set_limits(&self, tenant_id: Uuid, limits: ResourceQuotas) {
        let mut state = self.state.lock().unwrap();
        state.limits.insert(tenant_id, limits);
    }

    pub fn get_limits(&self, tenant_id: Uuid) -> Option<ResourceQuotas> {
        let state = self.state.lock().unwrap();
        state.limits.get(&tenant_id).cloned()
    }

    pub fn get_usage(&self, tenant_id: Uuid) -> ResourceQuotas {
        let state = self.state.lock().unwrap();
        state.usage.get(&tenant_id).cloned().unwrap_or_default()
    }

    pub fn reserve(&self, tenant_id: Uuid, request: &ResourceQuotas) -> Result<(), QuotaError> {
        let mut state = self.state.lock().unwrap();

        // Clone limits to drop the immutable borrow before mutating usage
        let limits = state
            .limits
            .get(&tenant_id)
            .cloned()
            .ok_or(QuotaError::LimitsNotConfigured)?;

        let usage = state.usage.entry(tenant_id).or_default();

        let mut projected_usage = usage.clone();
        projected_usage.add(request);

        if !projected_usage.fits_in(&limits) {
            return Err(QuotaError::LimitExceeded);
        }

        *usage = projected_usage;
        Ok(())
    }

    pub fn release(&self, tenant_id: Uuid, request: &ResourceQuotas) {
        let mut state = self.state.lock().unwrap();
        if let Some(usage) = state.usage.get_mut(&tenant_id) {
            usage.sub(request);
        }
    }
}

impl Default for QuotaEngine {
    fn default() -> Self {
        Self::new()
    }
}

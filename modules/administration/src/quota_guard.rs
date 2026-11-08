#![forbid(unsafe_code)]

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantQuotaLimits {
    pub max_domains: u32,
    pub max_databases: u32,
    pub max_mailboxes: u32,
    pub max_storage_bytes: u64,
    pub max_memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TenantCurrentUsage {
    pub domain_count: u32,
    pub database_count: u32,
    pub mailbox_count: u32,
    pub storage_bytes: u64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ResourceRequest {
    pub domains: u32,
    pub databases: u32,
    pub mailboxes: u32,
    pub storage_bytes: u64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaViolation {
    pub resource: String,
    pub requested_total: u64,
    pub limit: u64,
    pub remaining: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuotaExceededError {
    pub violations: Vec<QuotaViolation>,
}

impl std::fmt::Display for QuotaExceededError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Quota exceeded for one or more resources: {:?}", self.violations)
    }
}

impl std::error::Error for QuotaExceededError {}

pub struct TenantQuotaGuard;

impl TenantQuotaGuard {
    pub fn evaluate(
        limits: &TenantQuotaLimits,
        usage: &TenantCurrentUsage,
        request: &ResourceRequest,
    ) -> Result<(), QuotaExceededError> {
        let mut violations = Vec::new();

        let mut check = |requested_inc: u64, current: u64, limit: u64, name: &str| {
            let requested_total = current.saturating_add(requested_inc);
            if requested_total > limit {
                let remaining = limit.saturating_sub(current);
                violations.push(QuotaViolation {
                    resource: name.to_string(),
                    requested_total,
                    limit,
                    remaining,
                });
            }
        };

        check(request.domains as u64, usage.domain_count as u64, limits.max_domains as u64, "domains");
        check(request.databases as u64, usage.database_count as u64, limits.max_databases as u64, "databases");
        check(request.mailboxes as u64, usage.mailbox_count as u64, limits.max_mailboxes as u64, "mailboxes");
        check(request.storage_bytes, usage.storage_bytes, limits.max_storage_bytes, "storage_bytes");
        check(request.memory_bytes, usage.memory_bytes, limits.max_memory_bytes, "memory_bytes");

        if violations.is_empty() {
            Ok(())
        } else {
            Err(QuotaExceededError { violations })
        }
    }

    pub fn remaining_capacity(limits: &TenantQuotaLimits, usage: &TenantCurrentUsage) -> ResourceRequest {
        ResourceRequest {
            domains: limits.max_domains.saturating_sub(usage.domain_count),
            databases: limits.max_databases.saturating_sub(usage.database_count),
            mailboxes: limits.max_mailboxes.saturating_sub(usage.mailbox_count),
            storage_bytes: limits.max_storage_bytes.saturating_sub(usage.storage_bytes),
            memory_bytes: limits.max_memory_bytes.saturating_sub(usage.memory_bytes),
        }
    }
}

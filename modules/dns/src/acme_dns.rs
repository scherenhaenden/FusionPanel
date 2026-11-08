#![forbid(unsafe_code)]

use crate::domain::{DomainError, DomainName, Record, RecordType};
use crate::provider::{Plan, Provider, ProviderError, RecordChange};
use uuid::Uuid;

pub struct AcmeDnsManager<P: Provider> {
    provider: P,
}

impl<P: Provider> AcmeDnsManager<P> {
    pub fn new(provider: P) -> Self {
        Self { provider }
    }

    pub fn normalize_domain(&self, domain_name: &DomainName) -> Result<DomainName, DomainError> {
        let name_str = domain_name.as_str();
        let stripped = name_str.strip_prefix("*.").unwrap_or(name_str);
        DomainName::parse(&format!("_acme-challenge.{}", stripped))
    }

    pub fn add_challenge(
        &self,
        zone_id: Uuid,
        domain_name: &DomainName,
        token: &str,
    ) -> Result<(), ProviderError> {
        let normalized = self.normalize_domain(domain_name)?;
        let record = Record::new(
            normalized,
            RecordType::TXT,
            60,
            token.to_string(),
        )?;

        let plan = Plan::UpdateRecords(zone_id, vec![RecordChange::Add(record)]);
        let planned = self.provider.plan_change(plan)?;
        self.provider.apply_change(planned)?;

        Ok(())
    }

    pub fn verify_presence(
        &self,
        zone_id: Uuid,
        domain_name: &DomainName,
        token: &str,
    ) -> Result<bool, ProviderError> {
        let observed = self.provider.observe_zone(zone_id)?;
        let normalized = self.normalize_domain(domain_name)?;

        for record in observed.zone.records() {
            if record.name() == &normalized
                && matches!(record.record_type(), RecordType::TXT)
                && record.data() == token
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub fn check_propagation_readiness(
        &self,
        zone_id: Uuid,
        domain_name: &DomainName,
        token: &str,
    ) -> Result<bool, ProviderError> {
        self.verify_presence(zone_id, domain_name, token)
    }

    pub fn cleanup_challenge(
        &self,
        zone_id: Uuid,
        domain_name: &DomainName,
        token: &str,
    ) -> Result<(), ProviderError> {
        let normalized = self.normalize_domain(domain_name)?;
        let record = Record::new(
            normalized,
            RecordType::TXT,
            60,
            token.to_string(),
        )?;

        let plan = Plan::UpdateRecords(zone_id, vec![RecordChange::Remove(record)]);
        let planned = self.provider.plan_change(plan)?;
        self.provider.apply_change(planned)?;

        Ok(())
    }
}

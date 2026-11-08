#![forbid(unsafe_code)]

use dns::acme_dns::AcmeDnsManager;
use dns::backend::AuthoritativeBackend;
use dns::domain::{DomainName, TenantId, Zone};
use dns::provider::{Plan, Provider};

#[test]
fn test_normalize_domain() {
    let backend = AuthoritativeBackend::new();
    let manager = AcmeDnsManager::new(backend);

    let base = DomainName::parse("example.com").unwrap();
    let normalized = manager.normalize_domain(&base).unwrap();
    assert_eq!(normalized.as_str(), "_acme-challenge.example.com");

    let wildcard = DomainName::parse("*.example.com").unwrap();
    let normalized_wildcard = manager.normalize_domain(&wildcard).unwrap();
    assert_eq!(normalized_wildcard.as_str(), "_acme-challenge.example.com");
}

#[test]
fn test_acme_dns_manager_lifecycle() {
    let backend = AuthoritativeBackend::new();
    let manager = AcmeDnsManager::new(backend.clone());

    let domain = DomainName::parse("example.com").unwrap();
    let tenant_id = TenantId::new();
    let zone = Zone::new(domain.clone(), tenant_id);
    let zone_id = zone.id();

    let create_plan = Plan::CreateZone(zone);
    backend.plan_change(create_plan.clone()).unwrap();
    backend.apply_change(create_plan).unwrap();

    let token = "test-token-123";

    // verify not present initially
    let is_present = manager.verify_presence(zone_id, &domain, token).unwrap();
    assert!(!is_present);

    // add challenge
    manager.add_challenge(zone_id, &domain, token).unwrap();

    // verify present
    let is_present = manager.verify_presence(zone_id, &domain, token).unwrap();
    assert!(is_present);

    // check propagation readiness
    let ready = manager.check_propagation_readiness(zone_id, &domain, token).unwrap();
    assert!(ready);

    // check ttl is 60 (we can observe zone)
    let observed = backend.observe_zone(zone_id).unwrap();
    let records = observed.zone.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].ttl(), 60);
    assert_eq!(records[0].data(), token);

    // cleanup challenge
    manager.cleanup_challenge(zone_id, &domain, token).unwrap();

    // verify removed
    let is_present_after = manager.verify_presence(zone_id, &domain, token).unwrap();
    assert!(!is_present_after);
}

#[test]
fn test_acme_dns_manager_wildcard() {
    let backend = AuthoritativeBackend::new();
    let manager = AcmeDnsManager::new(backend.clone());

    let domain = DomainName::parse("example.com").unwrap();
    let wildcard = DomainName::parse("*.example.com").unwrap();
    let tenant_id = TenantId::new();
    let zone = Zone::new(domain.clone(), tenant_id);
    let zone_id = zone.id();

    let create_plan = Plan::CreateZone(zone);
    backend.plan_change(create_plan.clone()).unwrap();
    backend.apply_change(create_plan).unwrap();

    let token = "test-token-456";

    manager.add_challenge(zone_id, &wildcard, token).unwrap();

    let is_present = manager.verify_presence(zone_id, &wildcard, token).unwrap();
    assert!(is_present);

    // observe directly to ensure normalization applied
    let observed = backend.observe_zone(zone_id).unwrap();
    let records = observed.zone.records();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].name().as_str(), "_acme-challenge.example.com");

    manager.cleanup_challenge(zone_id, &wildcard, token).unwrap();
    let is_present_after = manager.verify_presence(zone_id, &wildcard, token).unwrap();
    assert!(!is_present_after);
}

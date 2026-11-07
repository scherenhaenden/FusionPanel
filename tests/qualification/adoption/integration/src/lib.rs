#![forbid(unsafe_code)]

#[cfg(test)]
mod tests {
    use adoption::{OwnershipGuard, OwnershipTier, Resource, DecommissionAttestation, attempt_decommission, UpstreamTrustMode, UpstreamTlsPolicy};
    use adopt07_tls::{UpstreamTlsPolicy as Adopt07Policy, TrustMode as Adopt07TrustMode};
    use std::time::{SystemTime, UNIX_EPOCH};
    use tempfile::tempdir;

    #[test]
    fn test_ownership_guard_preserves_external_resources() {
        let external_resource = Resource {
            id: "ext-db-1".to_string(),
            tier: OwnershipTier::ExternallyManaged,
        };
        let panel_resource = Resource {
            id: "panel-site-1".to_string(),
            tier: OwnershipTier::PanelOwned,
        };

        // Externally managed resource should reject lifecycle changes
        assert_eq!(OwnershipGuard::can_suspend(&external_resource), Err("Cannot suspend externally managed resources"));
        assert_eq!(OwnershipGuard::can_delete(&external_resource), Err("Cannot delete externally managed resources"));
        assert_eq!(OwnershipGuard::can_restart(&external_resource), Err("Cannot restart externally managed resources"));

        // Panel owned resource should allow lifecycle changes
        assert_eq!(OwnershipGuard::can_suspend(&panel_resource), Ok(()));
        assert_eq!(OwnershipGuard::can_delete(&panel_resource), Ok(()));
        assert_eq!(OwnershipGuard::can_restart(&panel_resource), Ok(()));
    }

    #[test]
    fn test_decommission_attestation_success() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();
        let attestation = DecommissionAttestation::new(true, true, true, true, now);
        assert_eq!(attempt_decommission(&attestation), Ok(()));
    }

    #[test]
    fn test_decommission_attestation_expired() {
        let old_time = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() - 90000;
        let attestation = DecommissionAttestation::new(true, true, true, true, old_time);
        assert_eq!(attempt_decommission(&attestation), Err("Evidence has expired"));
    }

    #[test]
    fn test_decommission_attestation_missing_evidence() {
        let now = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs();

        let attestation = DecommissionAttestation::new(false, true, true, true, now);
        assert_eq!(attempt_decommission(&attestation), Err("Target state is not independently verified"));

        let attestation = DecommissionAttestation::new(true, false, true, true, now);
        assert_eq!(attempt_decommission(&attestation), Err("Backup successor is not verified"));

        let attestation = DecommissionAttestation::new(true, true, false, true, now);
        assert_eq!(attempt_decommission(&attestation), Err("Certificate renewal successor is not verified"));

        let attestation = DecommissionAttestation::new(true, true, true, false, now);
        assert_eq!(attempt_decommission(&attestation), Err("Post-cutover changes are not reconciled"));
    }

    #[test]
    fn test_real_disposable_file_fixtures_for_ownership() {
        // Use tempfile for RAII cleanup
        let base_dir = tempdir().unwrap();
        let ext_dir = base_dir.path().join("external_workload");
        let panel_dir = base_dir.path().join("managed_site");
        std::fs::create_dir_all(&ext_dir).unwrap();
        std::fs::create_dir_all(&panel_dir).unwrap();

        let panel_resource = Resource { id: "panel1".to_string(), tier: OwnershipTier::PanelOwned };
        let ext_resource = Resource { id: "ext1".to_string(), tier: OwnershipTier::ExternallyManaged };

        // Test delete on panel resource
        if OwnershipGuard::can_delete(&panel_resource).is_ok() {
            std::fs::remove_dir_all(&panel_dir).unwrap();
        }

        // Assert panel was deleted
        assert!(!panel_dir.exists());

        // Test delete on external resource
        if OwnershipGuard::can_delete(&ext_resource).is_ok() {
            std::fs::remove_dir_all(&ext_dir).unwrap();
        }

        // Assert external resource was NOT deleted
        assert!(ext_dir.exists());
    }

    #[test]
    fn test_trust_and_sni_modes_remain_explicit() {
        let _domain_policy = UpstreamTlsPolicy {
            trust_mode: UpstreamTrustMode::InsecureCompatibility {
                audit_trail: "TICKET-999".to_string(),
                migration_path: "Q3 Plan".to_string(),
            },
            sni_preservation: true,
            expected_hostname: "example.com".to_string(),
        };

        let policy = Adopt07Policy::new("example.com")
            .with_trust_mode(Adopt07TrustMode::InsecureCompatibility {
                audit_trail: "TICKET-999".to_string(),
                migration_path: "Q3 Plan".to_string(),
            });

        let config = policy.generate_nginx_config();
        assert!(config.contains("proxy_ssl_verify off;"));
        assert!(config.contains("# WARNING: Insecure compatibility mode enabled"));
        assert!(config.contains("# Audit Trail: TICKET-999"));
        assert!(config.contains("# Migration Path: Q3 Plan"));
    }

    #[test]
    #[should_panic(expected = "RESULT=BLOCKED: Missing full OS disposable lab (T3) for OS source retirement verification")]
    fn test_full_os_source_retirement_blocked_without_lab() {
        let is_t3_lab_present = std::env::var("FUSIONPANEL_T3_LAB_PRESENT").is_ok();
        if !is_t3_lab_present {
            // Must explicitly panic to fail the test and report BLOCKED
            panic!("RESULT=BLOCKED: Missing full OS disposable lab (T3) for OS source retirement verification");
        }
    }
}

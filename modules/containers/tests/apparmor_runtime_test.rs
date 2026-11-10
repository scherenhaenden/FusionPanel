#![forbid(unsafe_code)]

use containers::apparmor_runtime::{ContainerSpec, RuntimeSecurityProfileManager, SecurityError};

#[test]
fn test_validate_spec_privileged() {
    let manager = RuntimeSecurityProfileManager::new();
    let spec = ContainerSpec {
        name: "test-privileged".to_string(),
        privileged: true,
        ..Default::default()
    };

    let result = manager.validate_spec(&spec);
    assert!(matches!(result, Err(SecurityError::PrivilegedContainer)));
}

#[test]
fn test_validate_spec_unconfined_apparmor() {
    let manager = RuntimeSecurityProfileManager::new();
    let spec = ContainerSpec {
        name: "test-unconfined".to_string(),
        apparmor_profile: Some("unconfined".to_string()),
        ..Default::default()
    };

    let result = manager.validate_spec(&spec);
    assert!(matches!(result, Err(SecurityError::UnconfinedAppArmor)));
}

#[test]
fn test_validate_spec_disabled_seccomp() {
    let manager = RuntimeSecurityProfileManager::new();

    let spec1 = ContainerSpec {
        name: "test-no-seccomp".to_string(),
        ..Default::default()
    };
    let result1 = manager.validate_spec(&spec1);
    assert!(matches!(result1, Err(SecurityError::SeccompDisabled)));

    let spec2 = ContainerSpec {
        name: "test-unconfined-seccomp".to_string(),
        seccomp_profile: Some("unconfined".to_string()),
        ..Default::default()
    };
    let result2 = manager.validate_spec(&spec2);
    assert!(matches!(result2, Err(SecurityError::SeccompDisabled)));
}

#[test]
fn test_validate_spec_dangerous_capability() {
    let manager = RuntimeSecurityProfileManager::new();
    let spec = ContainerSpec {
        name: "test-cap".to_string(),
        capabilities: vec!["CAP_SYS_ADMIN".to_string()],
        seccomp_profile: Some("docker-default".to_string()),
        ..Default::default()
    };

    let result = manager.validate_spec(&spec);
    match result {
        Err(SecurityError::DangerousCapability(cap)) => assert_eq!(cap, "CAP_SYS_ADMIN"),
        _ => panic!("Expected DangerousCapability error"),
    }
}

#[test]
fn test_audit_score_perfect() {
    let manager = RuntimeSecurityProfileManager::new();
    let spec = ContainerSpec {
        name: "test-perfect".to_string(),
        privileged: false,
        apparmor_profile: Some("docker-fusionpanel-test".to_string()),
        seccomp_profile: Some("docker-default".to_string()),
        capabilities: vec![],
    };

    let audit = manager.audit_score(&spec);
    assert_eq!(audit.score, 100);
    assert!(audit.violations.is_empty());
}

#[test]
fn test_audit_score_terrible() {
    let manager = RuntimeSecurityProfileManager::new();
    let spec = ContainerSpec {
        name: "test-terrible".to_string(),
        privileged: true, // -100 (saturating to 0)
        apparmor_profile: Some("unconfined".to_string()),
        seccomp_profile: None,
        capabilities: vec!["CAP_SYS_ADMIN".to_string()],
    };

    let audit = manager.audit_score(&spec);
    assert_eq!(audit.score, 0);
    assert_eq!(audit.violations.len(), 4); // Privileged, AppArmor, Seccomp, Cap
}

#[test]
fn test_audit_score_mixed() {
    let manager = RuntimeSecurityProfileManager::new();
    let spec = ContainerSpec {
        name: "test-mixed".to_string(),
        privileged: false,
        apparmor_profile: None, // -10
        seccomp_profile: Some("docker-default".to_string()),
        capabilities: vec!["CAP_NET_ADMIN".to_string()], // -5
    };

    let audit = manager.audit_score(&spec);
    assert_eq!(audit.score, 85);
    assert_eq!(audit.violations.len(), 2);
}

#[test]
fn test_generate_and_attach_profile() {
    let manager = RuntimeSecurityProfileManager::new();
    let mut spec = ContainerSpec {
        name: "web-app".to_string(),
        privileged: false,
        apparmor_profile: None,
        seccomp_profile: Some("docker-default".to_string()),
        capabilities: vec![],
    };

    manager.attach_profile(&mut spec);

    let attached_profile = spec
        .apparmor_profile
        .as_ref()
        .expect("Profile should be attached");
    assert_eq!(attached_profile, "docker-fusionpanel-web-app");

    let generated = manager.generate_profile(&spec);
    assert_eq!(generated.profile_name, "docker-fusionpanel-web-app");
    assert!(generated.profile_content.contains("deny /proc/kcore rwklx"));
    assert!(generated
        .profile_content
        .contains("deny /sys/firmware/** rwklx"));
    assert!(generated.profile_content.contains("deny ptrace"));
    assert!(generated.profile_content.contains("deny network raw"));
}

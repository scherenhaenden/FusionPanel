//! Executable case definitions for BLOCKED runtime prerequisites.
//! These tests are explicitly marked #[ignore = "BLOCKED: ..."]
//! to fulfill the requirement for executable case definitions without failing CI.

#[test]
#[ignore = "BLOCKED: Missing real fail2ban/pam_tally2 runtime environment (SEC-06)"]
fn test_brute_force_integration_blocked() {
    unimplemented!("Execute real brute-force attempt against a target daemon and verify lockout.");
}

#[test]
#[ignore = "BLOCKED: Missing namespace/container Nginx with ModSecurity loaded (SEC-07)"]
fn test_waf_integration_blocked() {
    unimplemented!("Send malicious traffic to the real WAF endpoint and verify it is blocked.");
}

#[test]
#[ignore = "BLOCKED: Missing namespace/cgroup sandbox setup and runner wrapper (SEC-11)"]
fn test_safe_command_execution_blocked() {
    unimplemented!("Execute a malicious command (e.g., escaping the sandbox) and ensure it fails safely.");
}

#[test]
#[ignore = "BLOCKED: Missing real OS adapters and CVE databases (SEC-14)"]
fn test_vulnerability_scanning_blocked() {
    unimplemented!("Scan a known vulnerable image and verify the CVEs are correctly identified.");
}

#[test]
#[ignore = "BLOCKED: Missing real sigstore/cosign integration (SEC-17)"]
fn test_release_integrity_blocked() {
    unimplemented!("Verify artifact signature against a mocked but valid public key/registry.");
}

#[test]
#[ignore = "BLOCKED: Missing writable audit log endpoint with cryptographic signing keys (SEC-13)"]
fn test_audit_integrity_blocked() {
    unimplemented!("Generate audit logs, sign them, and verify the signature out-of-band.");
}

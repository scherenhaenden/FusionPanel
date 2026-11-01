# Security Module

## Objective

Firewall/SSH/WAF/CSP typed policies, validation and safe apply plans.

## Capability Ownership and Status

- **SEC-05: Firewall management**: PARTIAL (Validation, types, and plan generation implemented; Missing: real runner integration and host process execution; follow-on item: "Implement ufw/iptables command generation and apply execution")
- **SEC-06: Brute-force protection**: BLOCKED (Missing behavior: Fail2ban/pam_tally2 integration definition; follow-on item: "Define brute-force types, metrics, and OS adapters")
- **SEC-07: Web application firewall**: PARTIAL (WAF policy types and validation implemented; Missing: Integration with actual Nginx/ModSecurity namespace proxies; follow-on item: "Implement WAF configuration generation for target proxies")
- **SEC-08: Bot protection**: BLOCKED (Missing behavior: Bot detection definitions; follow-on item: "Define bot protection rules and JS challenge integration")
- **SEC-09: SSH hardening**: PARTIAL (Strict type safety and sshd_config plan generation implemented; Missing: Real file writing, sshd reload, and recovery rollback; follow-on item: "Implement real SSH service modification with rollback")
- **SEC-10: Per-user isolation**: BLOCKED (Missing behavior: Namespace, cgroup, or chroot policies; follow-on item: "Define process and file isolation boundaries for tenants")
- **SEC-11: Safe command execution**: BLOCKED (Missing behavior: Runner wrapper and arg sanitization; follow-on item: "Implement secure process wrapper with restricted environments")
- **SEC-13: Tamper-evident audit integrity**: BLOCKED (Missing behavior: Audit log signing/shipping; follow-on item: "Implement write-only cryptographically signed audit logs")
- **SEC-14: Vulnerability scanning**: BLOCKED (Missing behavior: Scanner adapters and CVE databases; follow-on item: "Implement scanner bindings for OS packages and container images")
- **SEC-16: Content Security Policy controls**: PARTIAL (CSP types, validation, and generation implemented; Missing: Automated injection into web server configurations; follow-on item: "Bind CSP headers to site provisioning pipelines")
- **SEC-17: Release integrity and provenance**: BLOCKED (Missing behavior: Artifact signing/verification; follow-on item: "Implement sigstore/cosign integration for module updates")

## Testing and Verification

* All current partial capabilities (SEC-05, SEC-07, SEC-09, SEC-16) include unit tests covering type invariants and negative paths (e.g. invalid strings, insecure policies).
* Integration and End-to-End verified states are pending follow-up task scheduling for real system integration.

## Module Contracts

This is a standalone module package intended to be imported by the job executor or CLI.

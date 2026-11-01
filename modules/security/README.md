# Security Module

## Objective

Firewall/SSH/WAF/CSP typed policies, validation, and inert preview plans.

## Validation Contract

The policy APIs reject values that could alter the structure of generated
configuration previews. Validation happens before rendering; these previews are
data only and do not invoke host commands or modify system configuration.

- **SSH `AllowUsers`** accepts a conservative single-token pattern grammar:
  ASCII letters and digits plus `_`, `-`, `.`, `*`, and `?` for the user
  pattern, optionally followed by `@` and a host pattern using those characters
  plus `:`. Empty components, multiple `@` separators, whitespace, and control
  characters are rejected. The restricted grammar avoids directive injection
  and may reject patterns outside this supported subset.
- **Firewall source addresses** are represented by `IpNetwork`, not free-form
  strings. Parse addresses/CIDRs with `str::parse::<IpNetwork>()` or deserialize
  them as JSON strings. IPv4 and IPv6 prefix lengths are range checked, host bits
  are cleared, and the canonical value is used in the preview. A bare address
  represents one host. Malformed input, extra separators, and injected config
  text are rejected during parsing/deserialization.
- **WAF rule-group names** accept only non-empty ASCII letters, digits, `.`,
  `_`, and `-`. Whitespace, newlines, assignment separators, and other syntax
  characters are rejected before plan generation.

Unit tests exercise the accepted forms, canonicalization, and negative
directive-injection cases. The ignored integration fixtures are placeholders;
they do not demonstrate application to a real host. Runtime application,
rollback, firewall/SSH service reloads, and proxy WAF integration remain
unimplemented and must stay marked **BLOCKED** until an adapter exists and its
host-level integration and end-to-end behavior have been verified.

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
* Runtime application and host-level integration for SEC-05, SEC-07, and SEC-09 are **BLOCKED**: there is no executor/adapter, safe apply-and-rollback path, service reload, or proxy integration in this module.
* Integration and End-to-End behavior remain **BLOCKED** until those adapters are implemented and verified against isolated test hosts. The ignored integration fixtures are placeholders, not passing integration evidence.

## Module Contracts

This is a standalone module package intended to be imported by the job executor or CLI.

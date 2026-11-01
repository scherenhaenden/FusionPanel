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
  text are rejected during parsing/deserialization. Pass the configured SSH
  listen port to `FirewallPolicy::validate` and `generate_apply_plan`; neither
  API assumes port 22. The check requires a TCP allow rule covering that port
  when the default action is deny. It does not simulate firewall-provider rule
  ordering or establish live reachability.
- **WAF rule-group names** accept only non-empty ASCII letters, digits, `.`,
  `_`, and `-`. Whitespace, newlines, assignment separators, and other syntax
  characters are rejected before plan generation.
- **WAF modes** preserve `Standard`, `Strict`, and `CRSDetectionOnly` as
  selectable preview values. `CRSDetectionOnly` serializes with that stable
  name. The existing generic `DetectionOnly` and `Prevention` modes remain
  available. These are policy selections in an inert plan; they do not install
  or tune a WAF engine.

Unit tests exercise the accepted forms, canonicalization, configured SSH port,
WAF modes, and negative directive-injection cases. The WAF integration test
cases run and report **BLOCKED** with dependency probes; they do not count as
parser, traffic, or emergency-access passes. Other ignored integration fixtures
are placeholders. Runtime application, rollback, firewall/SSH service reloads,
and proxy WAF integration remain unimplemented and must stay marked **BLOCKED**
until an adapter exists and its host-level integration and end-to-end behavior
have been verified.

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
* WAF parser, benign/malicious traffic, and emergency SSH integration cases run as explicit **BLOCKED** diagnostics until the required disposable host adapters are available. Their test-harness success reports the blocker; it does not qualify the real integration or E2E capability.
* Integration and End-to-End behavior remain **BLOCKED** until those adapters are implemented and verified against isolated test hosts. The remaining ignored integration fixtures are placeholders, not passing integration evidence.

## Verification Evidence

Recorded run: 2026-11-01 16:26:00–16:26:01 Europe/Berlin, implementation commit
`40a7a429aa042559c1746cada8d186a00a1aac80`, Ubuntu 26.04.1 LTS,
`rustc 1.99.1 (58cf8f9c3 2026-10-13)`, and
`cargo 1.99.1 (5f94df478 2026-08-27)`.

### Commands and results

| Command | Result |
|---|---|
| `cargo fmt --manifest-path modules/security/Cargo.toml -- --check` | PASS, exit 0 |
| `git diff --check` | PASS, exit 0 |
| `cargo test --offline --manifest-path modules/security/Cargo.toml -- --nocapture --test-threads=1` | PASS for the runnable suite: 25 unit tests passed; 3 BLOCKED diagnostic tests ran and printed the missing runtime prerequisites; 5 other integration placeholders remain ignored; 0 failed. |
| `cargo tree --offline --manifest-path modules/security/Cargo.toml` | Direct Rust dependencies resolved offline: `serde 1.0.229`, `serde_json 1.0.154`, `thiserror 1.0.69`. No external WAF/parser dependency is declared or installed. |

The three diagnostic tests do not exercise a real parser, HTTP traffic, firewall,
or SSH service. Their test-harness status means only that the blocker report ran;
the corresponding integration and E2E outcomes remain **BLOCKED**, not PASS.

### Runtime dependency probes

| Probe | Observed result |
|---|---|
| `nginx -v` | NOT INSTALLED (`nginx: command not found`); `nginx -t` was not run. |
| `modsecurity --version` | NOT INSTALLED (`modsecurity: command not found`). |
| `sshd -V` | NOT INSTALLED (`sshd: command not found`). |
| `docker --version` | Docker 29.1.3, build `29.1.3-0ubuntu4.1`; daemon and container availability were not checked. |
| `dpkg-query -W -f='${binary:Package} ${Version}\n' libmodsecurity3` | Exit 1: no package found. |
| `dpkg-query -W -f='${binary:Package} ${Version}\n' libnginx-mod-http-modsecurity` | Exit 1: no package found. |
| `dpkg-query -W -f='${binary:Package} ${Version}\n' modsecurity-crs` | Exit 1: no package found. |
| `pkg-config --modversion libmodsecurity` | Could not query a version because `pkg-config` is not installed. |
| `iptables --version` | Exit 1: `iptables: Failed to initialize nft: Operation not permitted`; no firewall test was attempted. |

Accordingly, parser validation with Nginx/ModSecurity, benign and malicious HTTP
traffic, and emergency SSH access remain **BLOCKED** on this host. The exact
runtime requirements are Nginx with its ModSecurity connector, libmodsecurity
v3, OWASP CRS 3.x, a disposable network-isolated fixture, and an independently
reachable SSH recovery path. No fixture was created, no container or service was
started, no firewall or SSH configuration was applied, and teardown was not
applicable. Docker's CLI version alone does not establish that a daemon or safe
fixture is available.

## Module Contracts

This is a standalone module package intended to be imported by the job executor or CLI.

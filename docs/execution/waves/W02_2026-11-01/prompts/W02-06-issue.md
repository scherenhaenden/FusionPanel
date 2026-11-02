## Audit repair objective

Firewall lockout check ignores Deny rules/order/source: defaultAllow + Deny configuredSSH port passes; defaultDeny with earlier deny-all then allowSSH also passes. Define rule evaluation/administrator emergency-source semantics consistent with rendering. CSP validator checks exact unsafe-inline string; trailing whitespace/combined tokens bypass it; arbitrary nonce-/hash prefixes count as mitigation. Enforce grammar per token and complete valid nonce/hash syntax. Three integration_stubs.rs58-79 print BLOCKED and pass; change required qualification missing dependencies to nonzero BLOCKED and add actual isolated parser/traffic/emergency-access tests where available. No host firewall/SSH/service changes.

## Ownership

Exclusive write paths: modules/security/. Parent package S15, issue #23. Current verified master base 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba.

## Acceptance and delivery

Preserve feature IDs and selectable workflows. Include meaningful unit, real integration and E2E evidence; missing prerequisites are BLOCKED with nonzero status, not passing placeholders. Provide exact commands, versions, sanitized outputs and cleanup. Create/update a focused PR against master using Refs for this issue and its parent; never auto-close issues, merge, squash, force push or modify production/host services. No product capability acceptance from a merge alone.

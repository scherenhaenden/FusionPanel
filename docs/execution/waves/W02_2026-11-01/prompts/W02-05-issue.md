## Audit repair objective

Compare current merged PR85 with reviewed PR84 source f6df6f202323ec32a46cad5db353a4cd2e5a5ca0. Restore additively TTL0 support; valid Null MX priority/target and exclusion with other MX; distinct typed DNSKEY/DS key material; private invariant-bearing Record/Zone fields; numeric IPv4 alias/trailing-dot rejection. Preserve valid refined deserialization changes; do not overwrite the entire module with old code. Current domain.rs89,140,165,183,211,256/provider.rs6 show regressions. E2E must actually create/update/query/export/restore a disposable authoritative fixture, not merely check NOERROR from a hypothetical daemon; missing fixture is nonzero BLOCKED. Add focused regression cases.

## Ownership

Exclusive write paths: modules/dns/. Parent package S13, issue #16. Current verified master base 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba.

## Acceptance and delivery

Preserve feature IDs and selectable workflows. Include meaningful unit, real integration and E2E evidence; missing prerequisites are BLOCKED with nonzero status, not passing placeholders. Provide exact commands, versions, sanitized outputs and cleanup. Create/update a focused PR against master using Refs for this issue and its parent; never auto-close issues, merge, squash, force push or modify production/host services. No product capability acceptance from a merge alone.

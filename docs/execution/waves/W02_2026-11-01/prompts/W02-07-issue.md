## Audit repair objective

TCP probe currently uses only first resolved address (probes.rs43-44,67); try bounded candidates so reachable alternate addresses do not cause false incidents. Make timeout budgets deterministic and bounded across attempts. Existing integration.rs109-136 DOES use a loopback webhook receiver; preserve it and assert HTTP method/path/headers and JSON alert payload plus failure/recovery behavior. Do not claim unbounded default reqwest blocking timeout (it is30seconds). Add real local socket regressions and failure cases. Preserve explicit scheduler/collector/persistence/E2E gaps instead of inventing live metrics.

## Ownership

Exclusive write paths: modules/observability/. Parent package S16, issue #22. Current verified master base 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba.

## Acceptance and delivery

Preserve feature IDs and selectable workflows. Include meaningful unit, real integration and E2E evidence; missing prerequisites are BLOCKED with nonzero status, not passing placeholders. Provide exact commands, versions, sanitized outputs and cleanup. Create/update a focused PR against master using Refs for this issue and its parent; never auto-close issues, merge, squash, force push or modify production/host services. No product capability acceptance from a merge alone.

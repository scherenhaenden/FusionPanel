## Audit repair objective

Public deserializable plans bypass planning validation; revalidate all SQL identifiers/privileges at apply or make invariant-bearing types checked. PostgreSQL stdin SQL must set ON_ERROR_STOP and propagate SQL failures; current process status can report successful partial restores. Correct engine-specific database/schema/table grant semantics; do not use MariaDB table privileges in GRANT ON DATABASE. Fix ignored integration lane swallowing DependencyUnavailable and helper discarding results. Use disposable separate source/restore instances with real inserted records and grant authorization assertions; missing prerequisites must exit nonzero BLOCKED. Protect dumps/secrets and sanitized errors.

## Ownership

Exclusive write paths: modules/databases/. Parent package S10, issue #13. Current verified master base 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba.

## Acceptance and delivery

Preserve feature IDs and selectable workflows. Include meaningful unit, real integration and E2E evidence; missing prerequisites are BLOCKED with nonzero status, not passing placeholders. Provide exact commands, versions, sanitized outputs and cleanup. Create/update a focused PR against master using Refs for this issue and its parent; never auto-close issues, merge, squash, force push or modify production/host services. No product capability acceptance from a merge alone.

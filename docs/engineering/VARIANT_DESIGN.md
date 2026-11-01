# Selectable Workflow and Implementation Modes

## Principle

When multiple useful user-visible approaches exist for one capability, preserve them as explicit product modes. Offer selection at the narrowest sensible scope: account preference for presentation, site/stack policy for resource behavior, deployment policy for releases. Use neutral descriptive mode names. A different internal implementation does not automatically justify exposing a meaningless UI toggle.

## Initial mode inventory

| Capability | Modes to preserve/assess | Selection scope | Guardrails and evidence |
|---|---|---|---|
| Runtime hosting | Managed runtime / upstream reverse proxy | Site | Validate runtime availability, process supervision, ports, health and migration. |
| Databases | Host-managed / container-managed | Database service | One manager per instance; explicit data export/import and credential rewrite before switching. |
| Deployments | Release-directory switch / container release | App | Define shared storage, health, hooks and rollback; never promise rollback of arbitrary DB writes. |
| Container releases | In-place / blue-green | Deployment | Reject unsafe persistent-volume combinations; measure connection handling. |
| Build strategy | Dockerfile / buildpack / supplied artifact | App | Pin tools, isolate build credentials, preserve reproducible output and logs. |
| Backup strategy | Files plus DB / snapshot | Backup policy | Explain consistency and restore boundary; verify separate restore for each mode. |
| Restore scope | Full site / files only / database only / new domain | Restore request | Preview data overwrite, permissions and URL rewriting; require appropriate authorization. |
| TLS | HTTP challenge / DNS challenge / uploaded certificate | Domain | Provider permission, key custody, wildcard rules and renewal tests. |
| WAF | Standard / strict / CRS with detection-only rollout | Site | Exclusions are scoped and audited; use actual benign/malicious traffic tests. |
| Hosting operations | Guided transaction / advanced reviewed plan | Operator workflow | Same authorization and audit; privileged free-form shell is not a shortcut. |
| Migration | Fresh target / remote pull / uploaded export / in-place | Migration plan | Different conflict/recovery boundaries; complete dry-run and independent rollback first. |
| Frontend presentation | Guided / advanced detail | User preference | Same underlying capability and authorization; never hide critical errors or unsupported states. |

This is an initial inventory. Characterization must expand it when another materially different workflow is discovered. Themes and frontend frameworks are not substitutes for runtime feature parity.

## Registry contract

Represent a variant with capability_id, mode_id, schema_version, availability, scope, defaults, prerequisites, conflicts, persistence rules, change policy and acceptance_case_ids. Persist the selection with the resource, expose it in API/UI/CLI and export it in backups/IaC. Unsupported modes are explicit, not silently substituted. A per-user presentation preference must not change shared resource behavior.

Use stable mode IDs and versioned schemas. Validate compatibility on planning and again immediately before execution. Render unavailable choices with a reason. A capability's default can change only with a recorded migration policy; existing resource selections retain their meaning.

## Safe switching

Before a mode change: show current/target behavior, dry-run prerequisites and conflicts, explain data movement, obtain the appropriate operator action, acquire resource locks, back up where necessary, execute as a durable audited job, validate actual service/data health and record compensation. Mark non-reversible changes explicitly. Never claim a universal runtime toggle for data-store or ownership transitions.

## Qualification

Test every supported mode, required mode pairs and every incompatible combination. Prioritize critical pairwise combinations, then failure/data-loss boundaries; do not claim all combinations were tested when only pairwise coverage was run. Test saved preference roundtrip, export/import, restart persistence, safe transition failure and fallback rejection through real API and browser workflows.

# Domain Module Integration Contracts

## Packaging and versioning

Independent modules under `modules/<name>/` use their own Cargo.toml with `[workspace]`, version 0.1.0, edition 2021, AGPL-3.0-only and `#![forbid(unsafe_code)]`. They may not depend on unfinished siblings or change root workspace membership. Integration owns moving them into the shared workspace and the root lockfile after reviewing public types. Package-local README must describe APIs, tests, modes, supported/unsupported features and proposed shared changes.

Use serde request/result types, stable error enums and explicit validation. Resource identifiers use UUID-compatible strings at independent boundaries and are converted to canonical domain newtypes during integration. Distinguish Plan, AppliedResult and ObservedState. A successful plan is not an applied result. Reject unknown/unsupported modes; never return a fabricated success object.

## Operation contract

Inputs include operation/request ID, tenant/site/node identity, typed parameters, selected mode and expected state/version where relevant. Planning returns ordered actions, prerequisites, conflicts, required permissions, secret references and compensation boundaries. Runtime executes only after revalidation under the resource lock. Results include actual status, owned resource references, safe progress/events and reconciliation requirements. Errors distinguish invalid_input, unauthorized, conflict, unsupported, dependency_unavailable, timeout, operation_failed and reconciliation_required.

Do not place secret material in serializable events, plans, process arguments or fixtures. Use secret references and protected input mechanisms. Cancellation and retry policies are explicit; only retry idempotent steps without a reconciliation check. Jobs retain terminal outcomes and compensations. Record committed external effects and test interrupted execution.

## Domain-specific minimums

- Identity: capability + tenant/site scope, deny by default, revocation and session expiry enforced at use time. Bootstrap token is a temporary foundation mode, not multi-user authentication completion.
- Hosting: render typed site specs and staged configs; syntax-check before activation; atomic file replacement and service reload; one ownership model for users, ports and paths.
- Databases: engine and management mode explicit; real connection/grants/dump/restore APIs; validated identifiers and no shell interpolation.
- Backups: versioned manifest, consistent DB dump metadata, file checksums, retention, encryption/key references and separate-target restore.
- Containers: explicit labels for resource ownership; bounded lifecycle calls; inspect real volumes/networks/health before success.
- DNS: typed zone/record changes with provider capability matrix and conflict detection. Mail: explicit mailbox/relay scope, secret custody, config validation and actual delivery/IMAP evidence.
- Migration: read-only discovery, versioned IR, create/skip/conflict/unsupported dry-run and apply through ordinary services; preserve aliases/jobs/certs/ownership and mode selections.
- Frontend: real API errors/loading/permission state; selectable variants from registry, never hard-coded fake support. Any development fixture is explicitly marked and excluded from acceptance builds.

## Testing and handoff

Use real disposable dependencies for adapter integration and maintain unit tests of domain rules. Synthetic schema fixtures are valid for discovery tests, but production compatibility requires version-matched representative data. A module whose runtime is absent can deliver a tested plan/config API, with execution marked blocked. Report all capability gaps; do not mark a whole domain complete because one happy path passed.

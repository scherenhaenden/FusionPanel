# Migration Module

This standalone Rust module defines read-only inventory and dry-run planning
boundaries. It does not execute imports, alter files, create accounts, move
databases, change web-server configuration, or decommission a source host.

## Current qualification status

**No CloudPanel source release is currently qualified.** The public source
material reviewed for this change does not establish a versioned SQLite schema
fingerprint. Therefore `CloudPanelDiscovery::discover_with_version` fails
closed with `UnsupportedVersion` for every release until a sanitized,
version-matched source export is reviewed and an exact release is explicitly
registered. A plausible table layout or an operator-supplied version string
does not qualify a source release.

The Rust unit tests exercise a candidate SQLite layout with real SQLite. That
layout is a synthetic test fixture, not evidence of CloudPanel production
compatibility. It is used to validate read-only access, required-table checks,
association handling, missing-subsystem failures, and cleanup. The test-only
fixture adapter is not exposed as a production discovery API.

To qualify a release, add a sanitized export and its exact source package/CLI
version to a reviewable acceptance record, verify every table, column, and
association against that export, then register the exact supported version
with fixture and negative-drift tests. Keep credentials, private configuration,
production inventory, and raw operational findings out of this repository.

## Plan semantics and adoption gates

`PlanActionType::Create` means only “propose creating this resource after the
listed evidence has been reviewed.” The plan always reports `is_dry_run=true`
and `whole_server_apply_supported=false`. Calling `create_plan(..., false)`
returns `ApplyNotImplemented`; it cannot claim that a plan was applied.

The review planner returns `Conflict` for missing or contradictory evidence.
It requires:

- Source site metadata reconciled with effective server configuration.
- Custom routes, redirects, rewrites, discovery/OAuth endpoints, and
  streaming/WebSocket behavior preserved.
- Upstream listeners and TLS trust/SNI reviewed.
- Exactly one of strict managed-root validation or a separately approved
  external-root binding with an operator review reference.
- Database ownership, read-only export access, users, grants, and every
  site-to-database association reviewed.
- Existing target resources inspected before planning creates.
- Source schema qualification, independent database backup and restore, TLS
  renewal ownership, critical scheduled jobs, and external dependencies
  verified before decommission is allowed.
- Exact Linux usernames retained; invalid/option-like, reserved, or colliding
  names conflict. The planner never rewrites hyphens to underscores.

These are planner gates based on evidence supplied by collectors and operators.
This module does not itself parse effective Nginx configuration, probe an
upstream, validate a filesystem mount or symlink, create an OS account, inspect
backup execution, or test certificate renewal. Those runtime collectors and
their real dependency tests remain required before adoption or decommission
qualification. ADOPT-01 through ADOPT-07 remain incomplete as end-to-end product
capabilities.

## Capability ownership and continuity

| Capability | Status | Remaining acceptance work |
|---|---|---|
| MIG-01: CloudPanel import | BLOCKED | No apply path, revalidation, rollback, or real import outcome. |
| MIG-02: CloudPanel SQLite discovery | PARTIAL / BLOCKED | Candidate layout is covered by synthetic SQLite tests; no source release/schema has a version-matched qualification fixture. |
| MIG-03: HestiaCP import | BLOCKED | No discovery adapter or import path. |
| MIG-04: cPanel import | BLOCKED | No discovery adapter or import path. |
| MIG-05: Plesk import | BLOCKED | No discovery adapter or import path. |
| MIG-06: SSH migration | BLOCKED | No SSH transfer or authentication boundary. |
| MIG-07: Exporter-based migration | BLOCKED | Exporter protocol and daemon are undefined. |
| MIG-08: Migration dry-run | PARTIAL | Review-gated plan model exists; source collectors, complete target inspection, and accepted runtime evidence are missing. |
| MIG-09: Same-server migration | BLOCKED | No file movement, port remapping, or service transition. |
| MIG-10: Whole-server rollback | BLOCKED | No snapshot/backup integration or verified machine recovery. |
| MIG-11: DNS and mail migration | BLOCKED | No record transfer, mailbox parsing, or delivery/restore evidence. |
| MIG-12: Migration downtime control | BLOCKED | No live sync, proxy cutover, or measured downtime control. |

No row above represents production migration acceptance. Missing behavior must
remain visible and be linked to follow-up work; do not remove a capability from
the inventory because its first fixture passes.

Open acceptance follow-ups: [#73](https://github.com/scherenhaenden/FusionPanel/issues/73)
for source schema qualification, [#75](https://github.com/scherenhaenden/FusionPanel/issues/75)
for runtime route/config reconciliation, [#78](https://github.com/scherenhaenden/FusionPanel/issues/78)
for resource ownership, [#74](https://github.com/scherenhaenden/FusionPanel/issues/74)
for external roots and volumes, [#76](https://github.com/scherenhaenden/FusionPanel/issues/76)
for multi-database restore, [#80](https://github.com/scherenhaenden/FusionPanel/issues/80)
for backup and certificate-renewal handoff, [#77](https://github.com/scherenhaenden/FusionPanel/issues/77)
for OS identity mapping, and [#79](https://github.com/scherenhaenden/FusionPanel/issues/79)
for upstream TLS/SNI qualification.

## Tests

Run the standalone package suite:

```sh
cargo fmt --manifest-path modules/migration/Cargo.toml -- --check
cargo test --manifest-path modules/migration/Cargo.toml
```

The ordinary test suite includes unit/domain cases against real SQLite fixtures
and a negative SQLite integration case. Its success does not qualify a live
CloudPanel export or host migration.

The required source-export qualification lane is:

```sh
cargo test --manifest-path modules/migration/Cargo.toml --test e2e -- --ignored
```

Supply `FUSIONPANEL_CLOUDPANEL_DB` and
`FUSIONPANEL_CLOUDPANEL_VERSION` from a sanitized, version-matched export. The
test fails with `RESULT=BLOCKED` if the export/version is absent. It also fails
until an exact source version has a qualified schema adapter. Do not count an
ignored E2E case or the synthetic SQLite fixture as a passing source-server
qualification.

## Package and integration boundary

- Package-local tests run without joining the root Cargo workspace.
- Secrets do not belong in inventory metadata, fixtures, logs, or result bodies.
- Plans are distinct from applied results and observed host state.
- Root-path validation remains the responsibility of the strict hosting
  validator; an external-root approval is a separate binding mode.
- No shared root contracts or dependencies were changed by this module.

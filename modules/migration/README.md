# Migration Module

This module is responsible for migration discovery and planning.

## Capability Ownership and Continuity

The objective defines the initial bounded implementation slice.

- MIG-01: CloudPanel import - BLOCKED: Missing import routines and full execution context. Requires follow-on work for actual apply logic.
- MIG-02: CloudPanel SQLite discovery - IMPLEMENTED: Read-only SQLite version and inventory discovery.
- MIG-03: HestiaCP import - BLOCKED: Missing discovery and parsing for HestiaCP configuration formats.
- MIG-04: cPanel import - BLOCKED: Missing discovery and parsing for cPanel configuration formats.
- MIG-05: Plesk import - BLOCKED: Missing discovery and parsing for Plesk configuration formats.
- MIG-06: SSH migration - BLOCKED: Missing SSH transfer capability and authentication.
- MIG-07: Exporter-based migration - BLOCKED: Exporter daemon implementation and protocol not defined.
- MIG-08: Migration dry-run - IMPLEMENTED: Basic planner output generation for normalized inventory objects (skip, conflict, unsupported actions).
- MIG-09: Same-server migration - BLOCKED: File movement and port remapping logic not implemented.
- MIG-10: Whole-server migration rollback - BLOCKED: Snapshot/backup integration for whole server state rollback not implemented.
- MIG-11: DNS and mail migration - BLOCKED: DNS records transfer and mail directory parsing missing.
- MIG-12: Migration downtime control - BLOCKED: Real-time sync and proxy mechanisms not implemented.

## Supported/Unsupported Features
- Currently supports read-only SQLite database inspection for CloudPanel discovery (MIG-02).
- Returns normalized inventory for databases, sites, users.
- Plannner returns dry-run outputs.
- Does NOT execute any actual file changes, system migrations, or database dumps (dry-run only).
- ADOPT-02/03/05/06/07 and actual import execution are visibly BLOCKED in this bounded discovery PR.

## Proposed Shared Changes
- No shared root changes.

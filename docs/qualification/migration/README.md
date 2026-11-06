# Migration Qualification Evidence

This matrix tracks the real-system E2E execution of migration and rollback behaviors. Synthetic unit tests and mocks are strictly forbidden for qualification. Missing prerequisites must result in a `BLOCKED` test result, never a `PASS`.

## Qualification Matrix

| Capability ID | Description | Component Status | E2E Qualification Status |
|---|---|---|---|
| MIG-01 | CloudPanel import | S53 PARTIAL | **BLOCKED** (Missing required real SQLite database fixture) |
| MIG-02 | CloudPanel SQLite discovery | S53 PARTIAL | **BLOCKED** (Missing required real SQLite database fixture) |
| MIG-03 | HestiaCP import | PLANNED | BLOCKED (No source adapter behavior implemented) |
| MIG-04 | cPanel import | PLANNED | BLOCKED (No source adapter behavior implemented) |
| MIG-05 | Plesk import | PLANNED | BLOCKED (No source adapter behavior implemented) |
| MIG-06 | SSH migration | PLANNED | BLOCKED (No source adapter behavior implemented) |
| MIG-07 | Exporter-based migration | PLANNED | BLOCKED (No source adapter behavior implemented) |
| MIG-08 | Migration dry-run | S53 PARTIAL | **BLOCKED** (Missing required real SQLite database fixture) |
| MIG-09 | Same-server migration | PLANNED | BLOCKED (No specific component behavior implemented) |
| MIG-10 | Whole-server migration rollback | S53 PARTIAL | **BLOCKED** (Missing required real SQLite database fixture) |
| MIG-11 | DNS and mail migration | PLANNED | BLOCKED (No specific component behavior implemented) |
| MIG-12 | Migration downtime control | S53 PARTIAL | **BLOCKED** (Missing required real SQLite database fixture) |

## Test Execution Procedures

Run the standalone migration qualification tests using:

```bash
cargo test --manifest-path tests/e2e/migration/Cargo.toml
```

Constraints:
1. `tests/e2e/migration/` must be a standalone workspace to isolate dependencies from the global root.
2. Any test attempting to verify capabilities with a missing runtime tool (e.g. `sqlite3` daemon or specific source versions) MUST panic with `BLOCKED:` instead of passing.

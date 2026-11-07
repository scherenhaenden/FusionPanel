# BAK-05 — SFTP and FTPS destinations

**Priority** P3 (provisional) · **Score** 12.5% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

SFTP and FTPS destinations. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Implement this specific component behavior; then integrate S27, qualify real E2E S44 and close S56.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Partial component evidence | New partial component evidence at this baseline.  3 November source review: #241: Asynchronous SFTP storage operations; tests can pass without sshd and tolerate failed uploads, so round-trip/host-key acceptance remains unqualified. FTPS is not established. |
| Entire feature connected to product? | **Not established** | Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Implement this specific component behavior; then integrate S27, qualify real E2E S44 and close S56. |
| Owning work packages | S39, S27, S44, S56 | Recorded source paths: `modules/backup-destinations/`, `integration/data/`, `modules/databases/`, `modules/backups/`, `tests/e2e/data/`, `docs/qualification/data-network/`, `tests/closure/data-network/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/backup-destinations/tests/unit_local.rs`, `modules/backup-destinations/tests/unit_s3.rs`, `modules/backup-destinations/tests/unit_sftp.rs`, `modules/backup-destinations/tests/unit_sync.rs`. Historical ledger statement: tests can pass without sshd and tolerate failed uploads, so round-trip/host-key acceptance remains unqualified. then integrate S27, qualify real E2E S44 and close S56. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/data/tests/disposable_db.rs`, `integration/data/tests/unit_backup_scheduler.rs`, `integration/data/tests/unit_db_routes.rs`, `modules/backups/tests/integration.rs`, `modules/databases/tests/integration_postgres.rs`, `modules/databases/tests/integration_test.rs`, `modules/databases/tests/unit_dump.rs`, `modules/databases/tests/unit_health.rs`, `modules/databases/tests/unit_postgres.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: `tests/e2e/data/tests/multi_db_test.rs`. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 12.5 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **12.5%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

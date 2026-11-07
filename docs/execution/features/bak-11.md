# BAK-11 — Restore drills

**Priority** P3 (provisional) · **Score** 12.5% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Restore drills. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Complete and requalify this component slice; integrate S27, prove real E2E S44 and closure S56. Prior evidence is carried forward, not rerun.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Partial component evidence | PR #86 merged: database information is metadata only; one isolated file restore cycle was tested. Real database dumps or operator restore drills are not qualified. |
| Entire feature connected to product? | **Not established** | Complete and requalify this component slice; integrate S27, prove real E2E S44 and closure S56. Prior evidence is carried forward, not rerun. |
| Owning work packages | S11, S27, S44, S56 | Recorded source paths: `modules/backups/`, `integration/data/`, `modules/databases/`, `tests/e2e/data/`, `docs/qualification/data-network/`, `tests/closure/data-network/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/backups/tests/integration.rs`. Historical ledger statement: one isolated file restore cycle was tested. integrate S27, prove real E2E S44 and closure S56. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/data/tests/disposable_db.rs`, `integration/data/tests/unit_backup_scheduler.rs`, `integration/data/tests/unit_db_routes.rs`, `modules/backups/tests/integration.rs`, `modules/databases/tests/integration_postgres.rs`, `modules/databases/tests/integration_test.rs`, `modules/databases/tests/unit_dump.rs`, `modules/databases/tests/unit_health.rs`, `modules/databases/tests/unit_postgres.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: `tests/e2e/data/tests/multi_db_test.rs`. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 12.5 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **12.5%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

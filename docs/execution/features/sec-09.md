# SEC-09 — SSH hardening

**Priority** P2 (provisional) · **Score** 25% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

SSH hardening. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Complete and requalify this component slice; integrate S37, prove real E2E S52 and closure S57. Prior evidence is carried forward, not rerun.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Component gate credited | PR #71 merged: policy validation and inert previews, with 29 unit tests. Host application, real parsers, traffic, rollback, and emergency-access evidence are still required. Next owners: S37 / S52 / S57. |
| Entire feature connected to product? | **Not established** | Complete and requalify this component slice; integrate S37, prove real E2E S52 and closure S57. Prior evidence is carried forward, not rerun. |
| Owning work packages | S15, S37, S52, S57 | Recorded source paths: `modules/security/`, `integration/security/`, `tests/e2e/security/`, `docs/qualification/security/`, `docs/qualification/operations/`, `tests/closure/operations/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/security/tests/integration_stubs.rs`. Historical ledger statement: PR #71 merged: policy validation and inert previews, with 29 unit tests. integrate S37, prove real E2E S52 and closure S57. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/security/tests/security_tests.rs`, `modules/security/tests/integration_stubs.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: No test file located under the E2E-owner paths.. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 25 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **25%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

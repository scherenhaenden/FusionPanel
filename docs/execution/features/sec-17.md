# SEC-17 — Release integrity and provenance

**Priority** P2 (provisional) · **Score** 12.5% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Release integrity and provenance. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Deliver signed artifacts, establish trusted release keys and provenance, prove real install/update/recovery and tamper rejection through release E2E/closure.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Partial component evidence | Merged PR #131 updater includes Ed25519 signature verification and rollback helpers with reported mock-server tests; release publishing/provenance is not qualified. 3 November source review: #217: Signed updater executable permissions, candidate process health and rollback markers; disposable OS/service qualification remains. |
| Entire feature connected to product? | **Not established** | Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Deliver signed artifacts, establish trusted release keys and provenance, prove real install/update/recovery and tamper rejection through release E2E/closure. |
| Owning work packages | S15, S37, S52, S57 | Recorded source paths: `modules/security/`, `integration/security/`, `tests/e2e/security/`, `docs/qualification/security/`, `docs/qualification/operations/`, `tests/closure/operations/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/security/tests/integration_stubs.rs`. Historical ledger statement: Merged PR #131 updater includes Ed25519 signature verification and rollback helpers with reported mock-server tests; Original acceptance checklist (new source alone does not satisfy it): Deliver signed artifacts, establish trusted release keys and provenance, prove real install/update/recovery and tamper rejection through release E2E/closure. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/security/tests/security_tests.rs`, `modules/security/tests/integration_stubs.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: No test file located under the E2E-owner paths.. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 12.5 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **12.5%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

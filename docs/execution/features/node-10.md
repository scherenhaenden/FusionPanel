# NODE-10 — Control-state persistence

**Priority** P1 (provisional) · **Score** 12.5% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Control-state persistence. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Persist the complete resource/control model and prove consistent multi-process restart, reconciliation, upgrades and closure.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Partial component evidence | Merged identity SQLite storage (PR #90) and existing jobs store persist parts of control state; global configuration/resources are not integrated. 3 November source review: #227: SQLite tenant-owned resource store with revisions, WAL and reopen behavior; service composition remains. #238: SQLite node lifecycle/store and standalone API/agent executables; full-machine migration/HA is explicitly unqualified. |
| Entire feature connected to product? | **Not established** | Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Persist the complete resource/control model and prove consistent multi-process restart, reconciliation, upgrades and closure. |
| Owning work packages | S21, S34, S49, S57 | Recorded source paths: `modules/nodes/`, `integration/nodes/`, `tests/e2e/nodes/`, `docs/qualification/operations/`, `tests/closure/operations/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/nodes/target/debug/.fingerprint/nodes-445aee7a08b592c4/test-lib-nodes`, `modules/nodes/target/debug/.fingerprint/nodes-445aee7a08b592c4/test-lib-nodes.json`, `modules/nodes/target/debug/.fingerprint/nodes-6bc1b8edc2a134c3/test-integration-test-e2e`, `modules/nodes/target/debug/.fingerprint/nodes-6bc1b8edc2a134c3/test-integration-test-e2e.json`, `modules/nodes/target/debug/.fingerprint/nodes-8c7d3780cfbcf888/test-integration-test-integration`, `modules/nodes/target/debug/.fingerprint/nodes-8c7d3780cfbcf888/test-integration-test-integration.json`, `modules/nodes/target/debug/.fingerprint/nodes-b5ba18dc0d6f823a/test-integration-test-integration`, `modules/nodes/target/debug/.fingerprint/nodes-b5ba18dc0d6f823a/test-integration-test-integration.json`, `modules/nodes/target/debug/.fingerprint/nodes-b8853154488a0408/test-integration-test-unit_enrollment`, `modules/nodes/target/debug/.fingerprint/nodes-b8853154488a0408/test-integration-test-unit_enrollment.json`, `modules/nodes/target/debug/.fingerprint/nodes-d5b2a35b27f6bce8/test-integration-test-e2e`, `modules/nodes/target/debug/.fingerprint/nodes-d5b2a35b27f6bce8/test-integration-test-e2e.json`, `modules/nodes/target/debug/.fingerprint/nodes-db5899e1e8e255b7/test-lib-nodes`, `modules/nodes/target/debug/.fingerprint/nodes-db5899e1e8e255b7/test-lib-nodes.json`, `modules/nodes/tests/common/blocked_driver.rs`, `modules/nodes/tests/e2e.rs`, `modules/nodes/tests/integration.rs`, `modules/nodes/tests/unit_enrollment.rs`. Historical ledger statement: No feature-specific test result in prior ledger. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/nodes/tests/integration.rs`, `modules/nodes/target/debug/.fingerprint/nodes-445aee7a08b592c4/test-lib-nodes`, `modules/nodes/target/debug/.fingerprint/nodes-445aee7a08b592c4/test-lib-nodes.json`, `modules/nodes/target/debug/.fingerprint/nodes-6bc1b8edc2a134c3/test-integration-test-e2e`, `modules/nodes/target/debug/.fingerprint/nodes-6bc1b8edc2a134c3/test-integration-test-e2e.json`, `modules/nodes/target/debug/.fingerprint/nodes-8c7d3780cfbcf888/test-integration-test-integration`, `modules/nodes/target/debug/.fingerprint/nodes-8c7d3780cfbcf888/test-integration-test-integration.json`, `modules/nodes/target/debug/.fingerprint/nodes-b5ba18dc0d6f823a/test-integration-test-integration`, `modules/nodes/target/debug/.fingerprint/nodes-b5ba18dc0d6f823a/test-integration-test-integration.json`, `modules/nodes/target/debug/.fingerprint/nodes-b8853154488a0408/test-integration-test-unit_enrollment`, `modules/nodes/target/debug/.fingerprint/nodes-b8853154488a0408/test-integration-test-unit_enrollment.json`, `modules/nodes/target/debug/.fingerprint/nodes-d5b2a35b27f6bce8/test-integration-test-e2e`, `modules/nodes/target/debug/.fingerprint/nodes-d5b2a35b27f6bce8/test-integration-test-e2e.json`, `modules/nodes/target/debug/.fingerprint/nodes-db5899e1e8e255b7/test-lib-nodes`, `modules/nodes/target/debug/.fingerprint/nodes-db5899e1e8e255b7/test-lib-nodes.json`, `modules/nodes/tests/common/blocked_driver.rs`, `modules/nodes/tests/e2e.rs`, `modules/nodes/tests/integration.rs`, `modules/nodes/tests/unit_enrollment.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: `tests/e2e/nodes/target/debug/.fingerprint/e2e-nodes-89feee590f616732/test-lib-e2e_nodes`, `tests/e2e/nodes/target/debug/.fingerprint/e2e-nodes-89feee590f616732/test-lib-e2e_nodes.json`. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 12.5 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **12.5%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

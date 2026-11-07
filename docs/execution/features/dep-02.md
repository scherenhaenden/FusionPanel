# DEP-02 — Git webhooks

**Priority** P3 (provisional) · **Score** 25% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Git webhooks. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Wire a real authenticated webhook endpoint, persistence/replay controls, deployment trigger and real delivery E2E; close capability gates.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Component gate credited | Merged PR #136; modules/deployments/src/webhooks: GitHub/GitLab HMAC verification, W03 reports four unit tests. Revision 3: #195–#199/#203 consolidate build/webhook/release/workflow code. Real filesystem/git integration test source exists, but no fresh execution evidence or live traffic acceptance was obtained; existing gates retained. 3 November source review: #243: Deployment router uses in-memory HashMaps; webhook acceptance is not checkout/build, approvals are not demonstrated as authenticated identities, and health_failure is simulated. Durable restart, real health and traffic continuity remain. |
| Entire feature connected to product? | **Not established** | Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Wire a real authenticated webhook endpoint, persistence/replay controls, deployment trigger and real delivery E2E; close capability gates. |
| Owning work packages | S19, S30, S46, S55 | Recorded source paths: `modules/deployments/`, `integration/deployments/`, `tests/e2e/deployments/`, `docs/qualification/containers/`, `tests/closure/containers/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/deployments/tests/common/blocked_driver.rs`, `modules/deployments/tests/e2e.rs`, `modules/deployments/tests/integration.rs`, `modules/deployments/tests/unit_builder.rs`, `modules/deployments/tests/unit_domain.rs`, `modules/deployments/tests/unit_rollback.rs`, `modules/deployments/tests/unit_safety.rs`, `modules/deployments/tests/unit_switcher.rs`, `modules/deployments/tests/unit_webhooks.rs`. Historical ledger statement: modules/deployments/src/webhooks: GitHub/GitLab HMAC verification, W03 reports four unit tests. Real filesystem/git integration test source exists, but no fresh execution evidence or live traffic acceptance was obtained; Original acceptance checklist (new source alone does not satisfy it): Wire a real authenticated webhook endpoint, persistence/replay controls, deployment trigger and real delivery E2E; | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/deployments/tests/integration.rs`, `modules/deployments/tests/common/blocked_driver.rs`, `modules/deployments/tests/e2e.rs`, `modules/deployments/tests/integration.rs`, `modules/deployments/tests/unit_builder.rs`, `modules/deployments/tests/unit_domain.rs`, `modules/deployments/tests/unit_rollback.rs`, `modules/deployments/tests/unit_safety.rs`, `modules/deployments/tests/unit_switcher.rs`, `modules/deployments/tests/unit_webhooks.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: No test file located under the E2E-owner paths.. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 25 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **25%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

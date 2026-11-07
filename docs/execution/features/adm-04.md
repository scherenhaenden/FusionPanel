# ADM-04 — Panel updates and recovery

**Priority** P2 (provisional) · **Score** 12.5% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Panel updates and recovery. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Deliver genuine signed release installation, executable service units, complete update/recovery and real host E2E. No installer success credit until a real binary starts.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Partial component evidence | Merged PR #131/#132/#141; updater library/signature/rollback primitives and systemd definitions exist. Installer merely touches an empty binary; /opt vs /usr/bin paths disagree. Merged #182 updater E2E is an unconditional BLOCKED panic, not a working fixture. Installer/service blockers remain. 3 November source review: #217: Signed updater executable permissions, candidate process health and rollback markers; disposable OS/service qualification remains. #230: Versioned staged installer and systemd configuration; tests mock host commands and do not qualify a Debian install/reboot. #240: PostgreSQL password quoting, validated DomainName deserialization and installer systemctl error propagation. |
| Entire feature connected to product? | **Not established** | Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Deliver genuine signed release installation, executable service units, complete update/recovery and real host E2E. No installer success credit until a real binary starts. |
| Owning work packages | S20/S23, S36/S38, S51, S57 | Recorded source paths: `tests/e2e/administration/`, `docs/qualification/operations/`, `tests/closure/operations/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: No test file located under the component-owner paths.. Historical ledger statement: Merged #182 updater E2E is an unconditional BLOCKED panic, not a working fixture. tests mock host commands and do not qualify a Debian install/reboot. Original acceptance checklist (new source alone does not satisfy it): Deliver genuine signed release installation, executable service units, complete update/recovery and real host E2E. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: No test file located under the integration-owner paths.. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-4ab0faa83e466d99/test-integration-test-audit_logging`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-4ab0faa83e466d99/test-integration-test-audit_logging.json`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-4ed9f69374ffa9af/test-integration-test-tenant_quotas`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-4ed9f69374ffa9af/test-integration-test-tenant_quotas.json`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-bc792abaefdabd3f/test-integration-test-updater_rollback`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-bc792abaefdabd3f/test-integration-test-updater_rollback.json`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-d7966a5c3c5720cb/test-integration-test-invitations`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-d7966a5c3c5720cb/test-integration-test-invitations.json`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-e5a6fee0b1a6129a/test-lib-e2e_administration`, `tests/e2e/administration/target/debug/.fingerprint/e2e-administration-e5a6fee0b1a6129a/test-lib-e2e_administration.json`, `tests/e2e/administration/tests/audit_logging.rs`, `tests/e2e/administration/tests/common/mod.rs`, `tests/e2e/administration/tests/invitations.rs`, `tests/e2e/administration/tests/live_api/common.rs`, `tests/e2e/administration/tests/live_api_audit_logging.rs`, `tests/e2e/administration/tests/live_api_invitations.rs`, `tests/e2e/administration/tests/live_api_tenant_quotas.rs`, `tests/e2e/administration/tests/live_api_updater_rollback.rs`, `tests/e2e/administration/tests/tenant_quotas.rs`, `tests/e2e/administration/tests/updater_rollback.rs`. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 12.5 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **12.5%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

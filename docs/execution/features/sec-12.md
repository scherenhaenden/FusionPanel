# SEC-12 — Revocable sessions

**Priority** P1 (provisional) · **Score** 25% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Revocable sessions. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Integrate authenticated API issuance/enforcement, revocation/demotion across processes, real identity E2E and closure. TOTP/SSO/WebAuthn remain separate unsupported capabilities.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Component gate credited | Merged PR #90; modules/identity: principal-scoped RBAC, bounded sessions, SQLite persistence. README reports 17 unit + 5 real filesystem/SQLite integration tests. Merged #156 middleware exists; session login/refresh/logout runtime is still pending. No authenticated running-product evidence. 3 November source review: #224: SQLite identity adapter and persisted session/principal enforcement in standalone identity integration. |
| Entire feature connected to product? | **Not established** | Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Integrate authenticated API issuance/enforcement, revocation/demotion across processes, real identity E2E and closure. TOTP/SSO/WebAuthn remain separate unsupported capabilities. |
| Owning work packages | S08, S25, S52, S57 | Recorded source paths: `modules/identity/`, `integration/identity/`, `crates/api/`, `tests/e2e/security/`, `docs/qualification/security/`, `docs/qualification/operations/`, `tests/closure/operations/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/identity/tests/storage_test.rs`. Historical ledger statement: README reports 17 unit + 5 real filesystem/SQLite integration tests. 3 November source review: #224: SQLite identity adapter and persisted session/principal enforcement in standalone identity integration. Original acceptance checklist (new source alone does not satisfy it): Integrate authenticated API issuance/enforcement, revocation/demotion across processes, real identity E2E and closure. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/identity/tests/e2e_auth.rs`, `integration/identity/tests/unit_extractors.rs`, `integration/identity/tests/unit_middleware.rs`, `integration/identity/tests/unit_routes.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: No test file located under the E2E-owner paths.. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 25 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **25%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

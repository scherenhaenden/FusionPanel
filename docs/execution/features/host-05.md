# HOST-05 — Per-site PHP-FPM pools

**Priority** P4 (provisional) · **Score** 12.5% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Per-site PHP-FPM pools. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Complete real provisioning, OS ownership, parser and served-content checks, compensation, API/UI integration and S43/S54 acceptance.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Partial component evidence | PR #88 + merged #101; modules/hosting: checked configuration inputs and tenant pool mapping. Referenced artifact has since merged; runtime acceptance remains unqualified. 3 November source review: #234: Hosting compensation and failed-compensation marker; served PHP/Nginx and OS ownership acceptance remains. |
| Entire feature connected to product? | **Not established** | Qualify the merged behavior for this exact capability, complete its supported modes and authenticated product composition. Original acceptance checklist (new source alone does not satisfy it): Complete real provisioning, OS ownership, parser and served-content checks, compensation, API/UI integration and S43/S54 acceptance. |
| Owning work packages | S09, S26, S43, S54 | Recorded source paths: `modules/hosting/`, `integration/hosting/`, `tests/e2e/hosting/`, `docs/qualification/hosting/`, `tests/closure/hosting/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/hosting/tests/e2e.rs`, `modules/hosting/tests/integration.rs`, `modules/hosting/tests/unit_nginx_security.rs`, `modules/hosting/tests/unit_php_fpm_nginx.rs`, `modules/hosting/tests/unit_php_pool.rs`, `modules/hosting/tests/unit_static_site.rs`. Historical ledger statement: Original acceptance checklist (new source alone does not satisfy it): Complete real provisioning, OS ownership, parser and served-content checks, compensation, API/UI integration and S43/S54 acceptance. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/hosting/tests/acme_lifecycle.rs`, `integration/hosting/tests/live_hosting.rs`, `integration/hosting/tests/unit_acme.rs`, `integration/hosting/tests/unit_worker.rs`, `modules/hosting/tests/e2e.rs`, `modules/hosting/tests/integration.rs`, `modules/hosting/tests/unit_nginx_security.rs`, `modules/hosting/tests/unit_php_fpm_nginx.rs`, `modules/hosting/tests/unit_php_pool.rs`, `modules/hosting/tests/unit_static_site.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: `tests/e2e/hosting/tests/hosting_e2e.rs`. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 12.5 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **12.5%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

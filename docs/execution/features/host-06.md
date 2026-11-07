# HOST-06 — Let's Encrypt TLS

**Priority** P4 (provisional) · **Score** 12.5% as of 2026-11-06 · **Prior ledger state** PARTIAL · **Source reviewed** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`

## Planned user outcome

Let's Encrypt TLS. Canonical scope and original acceptance notes: [capability ledger](../CAPABILITY_LEDGER.md). The prior record says: Complete real signed ACME transport/order/validation/renewal, trusted certificate installation and actual hosted HTTPS verification; integrate S26/S43/S54.

## Code implementation

| Question | Status | Evidence |
|---|---|---|
| Feature-specific code credited? | Partial component evidence | Merged #175: integration/hosting/src/acme.rs contains ACME transport/client and HTTP-01 file helpers. Source artifact only; no real CA issuance proof. |
| Entire feature connected to product? | **Not established** | Complete real signed ACME transport/order/validation/renewal, trusted certificate installation and actual hosted HTTPS verification; integrate S26/S43/S54. |
| Owning work packages | S09, S26, S43, S54 | Recorded source paths: `modules/hosting/`, `integration/hosting/`, `tests/e2e/hosting/`, `docs/qualification/hosting/`, `tests/closure/hosting/` |

## Tests, separate from implementation

| Test type | Existing or historical test evidence | Result for source `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1` |
|---|---|---|
| Unit | Component-owner test source found: `modules/hosting/tests/e2e.rs`, `modules/hosting/tests/integration.rs`, `modules/hosting/tests/unit_nginx_security.rs`, `modules/hosting/tests/unit_php_fpm_nginx.rs`, `modules/hosting/tests/unit_php_pool.rs`, `modules/hosting/tests/unit_static_site.rs`. Historical ledger statement: Merged #175: integration/hosting/src/acme.rs contains ACME transport/client and HTTP-01 file helpers. | **Not individually rerun or mapped to this feature in this review.** |
| Integration | Integration-owner test source found: `integration/hosting/tests/acme_lifecycle.rs`, `integration/hosting/tests/live_hosting.rs`, `integration/hosting/tests/unit_acme.rs`, `integration/hosting/tests/unit_worker.rs`, `modules/hosting/tests/e2e.rs`, `modules/hosting/tests/integration.rs`, `modules/hosting/tests/unit_nginx_security.rs`, `modules/hosting/tests/unit_php_fpm_nginx.rs`, `modules/hosting/tests/unit_php_pool.rs`, `modules/hosting/tests/unit_static_site.rs`. File presence does not prove this feature is covered or that it passed. | **No feature-specific current run recorded.** |
| N2N / full end-to-end | E2E-owner test source found: `tests/e2e/hosting/tests/hosting_e2e.rs`. Source is not a passing product journey. | **No feature-specific current run recorded.** |
| Manual | No manual result cited in the feature ledger. | **No manual run recorded.** |

## Evidence gates

| Component | Integration | Real N2N | Closure | Score at 2026-11-06 |
|---:|---:|---:|---:|---:|
| 12.5 / 25 | 0 / 25 | 0 / 25 | 0 / 25 | **12.5%** |

The score records credited evidence across the four gates. It does not estimate code volume or engineering time. Current test outcomes remain unverified for this feature unless a specific run is listed above.

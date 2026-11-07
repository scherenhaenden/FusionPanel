# Product readiness snapshot — 7 November 2026

**Source baseline** `56ea5a18fbd215ad1085a1bb1c753bbdd5688fa1`, verified against GitHub master. **Previous scored baseline** `ab7cbf27d10abb7635730a6daeba8a590a65711e`. This is a focused review of the priority-1 journey and the main changes in the 49 first-parent commits and 109 changed files since the previous baseline.

## Priority and user outcome

| Priority | Outcome | Evidence in this cut | Next acceptance evidence |
|---|---|---|---|
| 1 · Core | Sign in, see real panel status and sign out | The identity router implements login, refresh and logout at both `/v1/auth/*` and `/api/v1/auth/*`. Four standalone identity route tests pass. The new dashboard requests health and capabilities. | Mount identity in the actual API, establish a persisted principal and browser-compatible session, then run the entire browser/API journey including revocation and restart. |
| 2 · Install and administer | Install the pilot on a clean Debian VM and repeat it after reboot | Installer and systemd files exist. Real clean installation, reboot and access management were not rerun for this review. | Run the pinned pilot on a disposable Debian VM and record the result. |
| 3 · Manage modules | Create and use a real MariaDB database | Database, backup, container and other module code changed. New source has not been qualified as an integrated product journey in this review. | Demonstrate browser/API/database creation, ownership, scoped SQL access, restart and tenant separation. |
| 4 · Websites | Create and serve a site | Changes include Nginx/PHP-FPM generators and WordPress code. | Demonstrate serving and failure recovery in a disposable environment. |

The current main API router exposes only `/v1/health` and `/v1/capabilities`. The latter checks an administrative Bearer token. The identity session router is not mounted there, so standalone route tests do not establish a browser login against the running product. The dashboard calls both status endpoints, but a browser-to-API success with an authenticated session was not demonstrated.

The identity handler now looks for a tenant in role bindings. If none is found it generates a new tenant ID, so the persisted-tenant requirement is still unqualified. Its session cookie sets `Secure`, which must be checked against the actual pilot origin. A current foundation test exercises the identity store directly alongside API health checks, and a separate case records missing identity flows. Neither establishes the requested browser journey.

## Checks performed

| Check | Result | Scope |
|---|---|---|
| `cd frontend && npm test -- --runInBand --watch=false` | PASS · 12 tests, 4 suites | Frontend tests. These do not establish the live product journey. |
| `cargo test --locked --offline --manifest-path integration/identity/Cargo.toml --test unit_routes` | PASS · 4 tests | Standalone route tests, including login and session invalidation. During invocation rustc also printed a SIGSEGV diagnostic. The command subsequently completed with exit code 0, so compiler stability warrants a clean rerun. |
| Browser login against actual API, restart and revocation | NOT_RUN | The required identity routes are absent from the main router. |
| Debian VM install, reboot and recovery | NOT_RUN | No disposable VM qualification was run in this review. |
| Real MariaDB, WordPress, DNS/mail, Docker and deployment journeys | NOT_RUN | Changed source and tests were not independently qualified here. |

## Progress scores and limits

The last complete scored catalog remains the [6 November snapshot](READINESS_SNAPSHOT_2026-11-06.json), with 8.02% for 162 capabilities, 43.49% for 63 packages, and 3 packages fully accepted at that baseline. Those are **historical figures**, not recalculated scores for 7 November. This review did not reconcile every changed feature and package or reassess prior acceptance. Use the previous catalog for per-row evidence and the current commit history for the change inventory.

The [product delivery plan](PRODUCT_DELIVERY_PLAN.md) states the current order. Its specific blocker text reflects the earlier `d005617` cut; the identity route and tenant details above supersede it for this snapshot. The existing [release decision](waves/W04_2026-11-03/final/RELEASE_DECISION.md) records NO-RELEASE. No new release qualification was performed.

[Complete feature and Arbeitspaket tables](READINESS_CATALOG_2026-11-07.md) · [Machine-readable catalog](READINESS_SNAPSHOT_2026-11-07.json)

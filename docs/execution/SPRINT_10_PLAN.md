# Sprint 10 Plan — Observability, Metrics & Alerts (`0.11.0-dev.1`)

## Objective

Deliver authenticated, bounded host telemetry from a node agent to the control plane, evaluate alert rules against actual samples, send HMAC-signed webhooks with delivery history, and display live metrics and alert configuration in the Dashboard.

This sprint covers the documented CPU, memory, disk-I/O, node/job health, alert threshold, signed webhook, and dashboard journey. It does not qualify every OBS capability: PagerDuty, Slack, ping/ICMP, per-site and per-container metrics, automatic recovery, public status pages, long-term time-series storage, and HA remain separate work.

## Baseline and branches

- Canonical base: `master` at `a8861e6110efaae7dbc589474335ca98c93a44c2` (Sprint 9 merge PR #695).
- Integration branch: `sprint-10`, created from that exact base.
- Target version: `0.11.0-dev.1`; bump it after the feature PRs are integrated and before final sprint qualification.
- Each feature has its own branch from `sprint-10`, focused changes, and local unit tests for the touched package/view. Feature PRs target `sprint-10`.
- After all feature PRs are reviewed and merged into `sprint-10`, bump the version, run the complete integration/browser/E2E/quality gates on the exact sprint head, document evidence and limitations, then open one `sprint-10` → `master` release PR. Do not merge the release PR as part of this work.

The existing work for issue #96 is already on this base: PR #364 merged at `403ddff1aa812e9f39c7bca8f047ec949bf133d4` and added bounded TCP candidate fallback plus loopback webhook request assertions. Do not create a duplicate fix branch. The product-level outage/incident/recovery E2E remains under #51.

## Atomic feature work

| Branch | Issue | Scope | Local unit gate |
|---|---|---|---|
| `feat/sprint-10-metrics-agent` | [#683](https://github.com/scherenhaenden/FusionPanel/issues/683) | Periodically collect actual node CPU, memory, disk-I/O, and job/agent health; report with node-scoped credentials; expose validated samples through a bounded, per-node ring-buffer API. Reuse the existing agent `MetricsCollector` and `system.info` contract where appropriate. | Agent metrics/worker tests and focused API ingestion/read authorization tests only. |
| `feat/sprint-10-alerts-webhook-api` | [#684](https://github.com/scherenhaenden/FusionPanel/issues/684), parent [#34](https://github.com/scherenhaenden/FusionPanel/issues/34) | Connect live samples to configurable threshold rules, persist rule and delivery state, send HMAC-signed webhooks with bounded timeout/retry behavior, and expose delivery history. Integrate with the main authenticated control-plane API; the separate observability example server is not product integration. | Focused alert evaluator, signing, persistence, authorization, and failure/retry tests for touched packages. |
| `feat/sprint-10-dashboard-metrics-ui` | [#685](https://github.com/scherenhaenden/FusionPanel/issues/685) | Show current node telemetry and recent bounded history; allow operators to manage supported alert rules and webhook targets; show loading, stale, empty, unauthorized, and API error states. | Focused Dashboard and API-client frontend unit tests. |

The E2E work item is [#51](https://github.com/scherenhaenden/FusionPanel/issues/51) (S48). It is a sprint-close gate, not a substitute for feature-level unit tests. Keep the existing loopback TCP/webhook coverage in #96 and add the real control-plane flow only once the three feature slices are integrated.

## Sprint-close acceptance

On the fully integrated and version-bumped `sprint-10` candidate:

1. A registered agent submits fresh node telemetry using its own node credential; missing, invalid, cross-node, malformed, non-finite, or oversized samples are rejected.
2. The API retains only the configured bounded window per node and returns it only to an authorized operator. Agent credentials cannot read the dashboard API.
3. The Dashboard displays live CPU, memory, disk-I/O, and node/job health with sample timestamps; it reports stale/offline data clearly.
4. A real threshold crossing creates one durable alert/delivery record and sends a signed webhook. The receiver verifies the signature and payload. Failure is recorded, retry remains bounded, and recovery resolves the incident without a false healthy transition.
5. Existing login, RBAC, jobs, and node workflows continue to work. No credential, signing secret, or private payload material appears in logs or API errors.
6. Update `SPRINTS.md`, this plan, and `PR_ISSUE_TRACKER.md` with the exact candidate SHA, commands, observed results, limits, and remaining OBS capabilities before opening the release PR.

## Resource-safe test sequence

- Feature branches: local unit tests for changed crates/views only; no workspace matrix, Docker, `act`, or live E2E per feature.
- Before any Cargo matrix, confirm at least 10 GiB free on the build-target filesystem. Use one workload at a time and stop if free-space or memory preflight fails.
- Before Docker/Compose/Playwright/Act, check free space on `/`, `/tmp`, and the Docker data-root filesystem; inspect `docker system df` and record `docker volume ls`. Do not use global prune commands. Clean up only fixture-owned resources after checking references.
- Sprint close: run the focused integration and live E2E gates on the exact version-bumped sprint head, then the required CI/quality gates. Record a missing prerequisite as `BLOCKED`; do not count it as a pass.

## Scope boundaries

- Reuse `modules/observability/` and `integration/observability/`; do not duplicate the already-merged #96 TCP fallback or its local webhook tests.
- The existing `crates/agent/src/metrics.rs` collector currently feeds `system.info`; periodic authenticated publication and the main API ring-buffer route are still needed.
- The existing `integration/observability` HTTP process is a standalone package and is not mounted by `fusionpanel-api`. Its unauthenticated `/metrics` behavior is not the new product endpoint contract.
- Keep unsupported notification providers and broader observability capabilities explicit in UI/API and the capability ledger.

# Sprint 10 Plan — Observability, Metrics & Alerts (`0.11.0-dev.1`)

## Objective

Deliver authenticated, bounded host telemetry from a node agent to the control plane, evaluate alert rules against actual samples, monitor product-managed HTTP uptime targets, send HMAC-signed webhooks with delivery history, and display live metrics and alert configuration in the Dashboard.

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
| `feat/sprint-10-metrics-agent` | [#683](https://github.com/scherenhaenden/FusionPanel/issues/683) | Periodically collect actual node CPU, memory and disk-I/O; report with node-scoped credentials; expose validated samples through a bounded, per-node ring-buffer API. Node/job health comes from existing control-plane records. Reuse the existing agent `MetricsCollector` and `system.info` contract where appropriate. | Agent metrics/worker tests and focused API ingestion/read authorization tests only. |
| `feat/sprint-10-alerts-webhook-api` | [#684](https://github.com/scherenhaenden/FusionPanel/issues/684), parent [#34](https://github.com/scherenhaenden/FusionPanel/issues/34) | Connect live samples to configurable threshold rules, persist rule and delivery state, send HMAC-signed webhooks with bounded timeout/retry behavior, and expose delivery history. Integrate with the main authenticated control-plane API; the separate observability example server is not product integration. | Focused alert evaluator, signing, persistence, authorization, and failure/retry tests for touched packages. |
| `feat/sprint-10-dashboard-metrics-ui` | [#685](https://github.com/scherenhaenden/FusionPanel/issues/685) | Show current node telemetry and recent bounded history; allow operators to manage supported alert rules and webhook targets; show loading, stale, empty, unauthorized, and API error states. | Focused Dashboard and API-client frontend unit tests. |
| `feat/sprint-10-uptime` | [#51](https://github.com/scherenhaenden/FusionPanel/issues/51) | Add authenticated product-managed HTTP uptime targets, bounded checks and incidents, and signed outage/recovery alert transitions. | Focused API lifecycle, validation, measured loopback outage/recovery tests, and observability persistence tests. |

Issue [#51](https://github.com/scherenhaenden/FusionPanel/issues/51) (S48) covers the uptime feature and its sprint-close product-process acceptance. Feature-level unit tests do not replace the process-level measured outage/recovery run. Keep the existing loopback TCP/webhook coverage in #96.

## Integrated feature checkpoint — 19 November 2026

All three atomic feature PRs have been reviewed and merged into `sprint-10`:

| PR | Feature | Sprint merge SHA | Focused local evidence |
|---|---|---|---|
| [#697](https://github.com/scherenhaenden/FusionPanel/pull/697) | Agent metrics publication and bounded API | `e4d142a24a3993a32232115fef526f808fa35276` | API metrics 4/4; agent metrics report 1/1; agent collector 1/1. |
| [#698](https://github.com/scherenhaenden/FusionPanel/pull/698) | Persistent alert rules and signed webhook delivery | `78ad9eeb0648eb46ae6cb2421a5c6bc9e4bcdc7b` | API alerts 5/5; API metrics 4/4; alert-store 2/2. Delivery tests use ephemeral loopback only. |
| [#699](https://github.com/scherenhaenden/FusionPanel/pull/699) | Dashboard telemetry and alert management | `c0e2762a6173fce559504a81dbfffbf07980b403` | Dashboard/API/routes 3 suites, 14 tests passed with mocked API responses. |

The version bump was integrated by [PR #700](https://github.com/scherenhaenden/FusionPanel/pull/700) at `347521e6c49807af1df4119fa0649e7d479dec8e`. Product live verification and CI fixes were then added through [PR #701](https://github.com/scherenhaenden/FusionPanel/pull/701), [PR #702](https://github.com/scherenhaenden/FusionPanel/pull/702), and [PR #703](https://github.com/scherenhaenden/FusionPanel/pull/703). The earlier versioned integration candidate was `752ebbd4265b7073278bd063d82fae100a3c86d7` (`0.11.0-dev.1`). PR #705 later added managed HTTP uptime without another version bump; its merge commit is `7f8883755679d40dbe693dea73c5bb7fe61a7556`.

## Earlier versioned candidate results — 19 November 2026

| Gate | Result on `752ebbd4265b7073278bd063d82fae100a3c86d7` | Status |
|---|---|---|
| Feature slices | Agent metrics/API, signed alert rules, and Dashboard implementation are integrated through PRs #697–#699. | Complete |
| Version | Workspace and nested agent protocol metadata report `0.11.0-dev.1` through PR #700. | Complete |
| Workspace unit/integration suite | `CARGO_BUILD_JOBS=2 cargo test --locked --offline --workspace` — 151 passed, 0 failed. | Pass |
| Agent tests | `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path crates/agent/Cargo.toml` — 51 passed, 0 failed. | Pass |
| Observability tests | `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path modules/observability/Cargo.toml` — 36 passed, 0 failed. | Pass |
| Secrets tests | `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path modules/secrets/Cargo.toml` — 31 passed, 0 failed; 1 isolated systemd/machine-restore test ignored because the required fixture is not available. | Pass with explicit blocker |
| Observability E2E package | `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path tests/e2e/observability/Cargo.toml` — 2 passed, 0 failed; 1 external SMTP test ignored. Separate live product lane passed 1/1 with the exact candidate API and agent binaries. | Pass for exercised cases |
| Frontend | `CI=true npm test -- --runInBand` — 11 suites / 48 tests passed; `npm run build` passed. | Pass |
| Format, Clippy, builds | Root formatting check; root, agent, observability, secrets, and E2E Clippy with `-D warnings`; API and agent binary builds. | Pass |
| Product Dashboard in browser | No browser E2E against a disposable integrated API + real agent fixture was run. Jest uses mocked API responses. | BLOCKED |
| Measured outage and recovery | The product live test uses a real agent sample to trigger a signed alert, then a synthetic low sample to resolve it after API restart. It does not observe a real service outage or measured uptime recovery. | BLOCKED under #51 |

The local Rust tests that bind sockets passed when run with the narrow permission needed for loopback/Unix socket operations. Cargo workloads were run serially with `CARGO_BUILD_JOBS=2` and offline dependencies. No Docker, Playwright, Act, external webhook, external SMTP, or host systemd workload was started.

## Product HTTP uptime implementation — PR #705 — 19 November 2026

PR [#705](https://github.com/scherenhaenden/FusionPanel/pull/705) merged from `feat/sprint-10-uptime`, based on `sprint-10` at `834ad4f02329eeffeef5fa365bf4993ea3030df5`. Implementation commit: `e7730d3218bdda837f96c9eda2d6f4d2c1cf262f`; review-fix commit: `a697215ec6843db5562a3fcbd2c9c7260c9fe260`; regular merge commit: `7f8883755679d40dbe693dea73c5bb7fe61a7556`. It adds authenticated target management, a scheduled product API collector, bounded SQLite check/incident history, and outage/recovery evaluation through signed alert rules. No version bump was requested, consistent with the Sprint 10 feature-then-qualification plan.

- `cargo fmt --all -- --check` and `git diff --check` passed.
- `cargo test --manifest-path modules/observability/Cargo.toml --offline -j 2 alert_store::tests::` — 4 passed.
- `cargo test --manifest-path crates/api/Cargo.toml --locked --offline -j 2 alerts::tests:: -- --test-threads=2` — 8 passed, including a local 503 → 200 measured probe, persisted incident recovery, and verified signed trigger/resolution webhook deliveries.
- The test fixtures used ephemeral loopback listeners only. This is feature-level local evidence; it is not a process-level product E2E or Dashboard browser acceptance. Keep #51 open until the measured outage/recovery E2E passes on the sprint candidate.

Current `sprint-10` head: `7f8883755679d40dbe693dea73c5bb7fe61a7556` (`0.11.0-dev.1`). The full release gate above ran on earlier candidate `752ebbd4265b7073278bd063d82fae100a3c86d7` and must be repeated on this current head.

## S48 capability status and follow-on work

The S48 capability register below preserves the broader observability domain; the Sprint 10 release acceptance covers only its integrated subset.

| Capability | Status on current sprint head after PR #705 | Follow-on / limitation |
|---|---|---|
| OBS-01 CPU, RAM, disk metrics | Partial product integration: the agent publishes CPU, memory, and disk-I/O samples to the authenticated API; the S48 module also contains additional primitives. | Network traffic metrics are not part of the product API sample. |
| OBS-02 Metrics history | Partial: the product API keeps a bounded 120-sample in-memory ring per node. | History is lost on API restart; durable time-series storage is separate work. |
| OBS-03 Live network traffic | Primitive only in the observability module. | Not exposed through the product API or Dashboard. |
| OBS-04 Per-site metrics | Primitive only in the observability module. | No product collector/API/UI path. |
| OBS-05 Per-container metrics | Primitive only in the observability module. | No product collector/API/UI path. |
| OBS-06 HTTP uptime checks | Product-managed targets and persisted checks are integrated by PR #705; feature-level measured outage/recovery tests pass. | Product-process measured outage-to-alert-to-recovery acceptance remains pending under #51. |
| OBS-07 TCP and ping checks | TCP primitive exists; ping is blocked on raw ICMP permissions. | No integrated product target management; ping remains unavailable. |
| OBS-08 Prometheus endpoint | Standalone sample endpoint only. | Not mounted by `fusionpanel-api`. |
| OBS-09 Configurable alerts | Implemented for CPU, memory, disk-I/O, and HTTP-uptime transitions with persisted rules/delivery history and signed webhooks. | External providers remain outside this slice. |
| OBS-10 PagerDuty / OBS-11 Slack | Blocked / not implemented. | Provider configuration and delivery integrations are not in Sprint 10 scope. |
| OBS-12 Automatic recovery | Blocked / not implemented. | No remediation engine or action policy. |
| OBS-13 System diagnostics | Blocked / not implemented. | No diagnostic toolchain integration. |
| OBS-14 Public status page | Blocked / not implemented. | No public status rendering/API. |
| OBS-15 Incident management | Partial: alert threshold state, uptime incidents, and delivery history persist. | Cross-process measured outage/recovery remains unqualified. |

Next work items before release qualification:

1. Verify the measured outage/recovery flow with a disposable product-process fixture on current `sprint-10`. Feature tests cover target lifecycle, failure, recovery, and signed alert delivery; they do not close #51.
2. Add a disposable browser fixture that starts the integrated API, real agent, and Dashboard together, then verifies live samples, stale/offline states, alert creation, signed delivery feedback, and safe teardown. This is the missing browser acceptance for #685.
3. Re-run the release gate on one exact candidate after those changes, update this plan and the tracker, then open the single `sprint-10` → `master` release PR only when the documented S48 acceptance is green.

Issues #683–#685, #34, and #51 remain open until their acceptance is verified on `master` and the merged PR SHA and actual evidence are recorded.

## Sprint-close acceptance

On the fully integrated and version-bumped `sprint-10` candidate:

1. A registered agent submits fresh node telemetry using its own node credential; missing, invalid, cross-node, malformed, non-finite, or oversized samples are rejected.
2. The API retains only the configured bounded window per node and returns it only to an authorized operator. Agent credentials cannot read the dashboard API.
3. The Dashboard displays live CPU, memory, disk-I/O, and node/job health with sample timestamps; it reports stale/offline data clearly.
4. A real threshold crossing creates one durable alert/delivery record and sends a signed webhook. The receiver verifies the signature and payload. Failure is recorded, retry remains bounded, and recovery resolves the incident without a false healthy transition. The current product live lane verifies sample-triggered alerts and synthetic threshold resolution; measured service uptime/outage recovery remains BLOCKED until the product-process fixture passes on the integrated sprint head.
5. Existing login, RBAC, jobs, and node workflows continue to work. No credential, signing secret, or private payload material appears in logs or API errors.
6. Update `SPRINTS.md`, this plan, and `PR_ISSUE_TRACKER.md` with the exact candidate SHA, commands, observed results, limits, and remaining OBS capabilities before opening the release PR. The earlier full-gate candidate is recorded above, and PR #705 adds focused evidence on the current head; because browser and measured outage acceptance remain pending, no release PR is opened yet.

## Resource-safe test sequence

- Feature branches: local unit tests for changed crates/views only; no workspace matrix, Docker, `act`, or live E2E per feature.
- Before any Cargo matrix, confirm at least 10 GiB free on the build-target filesystem. Use one workload at a time and stop if free-space or memory preflight fails.
- Before Docker/Compose/Playwright/Act, check free space on `/`, `/tmp`, and the Docker data-root filesystem; inspect `docker system df` and record `docker volume ls`. Do not use global prune commands. Clean up only fixture-owned resources after checking references.
- Sprint close: run the focused integration and live E2E gates on the exact version-bumped sprint head, then the required CI/quality gates. Record a missing prerequisite as `BLOCKED`; do not count it as a pass.

## Scope boundaries

- Reuse `modules/observability/` and `integration/observability/`; do not duplicate the already-merged #96 TCP fallback or its local webhook tests.
- PR #697 connects the existing agent collector to authenticated periodic publication and adds the main API ring-buffer route. The API history is in memory and is cleared on restart; no durable time-series storage is claimed.
- The existing `integration/observability` HTTP process is a standalone package and is not mounted by `fusionpanel-api`. Its unauthenticated `/metrics` behavior is not the new product endpoint contract.
- Keep unsupported notification providers and broader observability capabilities explicit in UI/API and the capability ledger.

# Sprint 2 Plan & Handoff Guide — Node Agent & Jobs Runtime

**Sprint Branch**: `sprint-2` (branched from `master` @ `6b5adf5`, `v0.2.0-dev.1`)  
**Target Release**: PR [#590](https://github.com/scherenhaenden/FusionPanel/pull/590) from `sprint-2` to `master` is open for release review; Cargo workspace package version `0.3.0-dev.1` is internal metadata, and no date/hash release tag or published artifact is recorded.<br>
**Docker Required**: **No** (runs natively with `fusionpanel-api`, `fusionpanel-agent`, `fusionpanel-cli`, and SQLite)  
**Estimated Effort**: L (8 story points)  

---

## 1. Sprint Goal (100% E2E Deliverable)

Enable the FusionPanel control plane to **register a local node, enqueue jobs via API/UI, claim and execute them on `fusionpanel-agent`, and persist execution status and logs in SQLite**. Process-restart persistence is a separate qualification requirement.

### What Works at Sprint 2 Close
1. Local `fusionpanel-agent` registers with `fusionpanel-api` and sends periodic heartbeats.
2. Authenticated operators (cookie session or Bearer token) can list nodes, enqueue a job, poll/view job status (`queued` -> `running` -> `succeeded` / `failed`), and inspect job logs from both the REST API and React UI.
3. SQLite store tests verify that node records, queued/completed jobs, and logs survive reopening `FUSIONPANEL_DATABASE_URL`; process-restart behavior remains a separate E2E qualification.

### Upgrade Compatibility

The agent defaults to HTTP mode. Existing deployments that use the legacy WebSocket transport must set `FUSIONPANEL_AGENT_MODE=legacy-ws` before upgrading and retain `FUSIONPANEL_AGENT_URL` and `FUSIONPANEL_AGENT_SECRET`; `FUSIONPANEL_AGENT_HEARTBEAT_URL` remains optional. Remote legacy WebSocket endpoints must use `wss://`; plaintext `ws://` is accepted only for loopback and localhost endpoints. The default heartbeat endpoint remains local `ws://127.0.0.1:8080/heartbeat`. If any legacy WebSocket environment variable is present while the mode is unset, the agent exits with a configuration error rather than selecting WebSocket implicitly.

---

## 2. Atomic Branches & Handoff Task Breakdown

Each branch **must** be developed independently from `sprint-2`, run **only local unit tests** for touched crates/views, and open a **PR targeting `sprint-2`**.

### Task 2.1 — `feat/sprint-2-node-jobs-api` (Control Plane API: `/v1/nodes` & `/v1/jobs`)
* **Primary Paths**:
  * `crates/api/Cargo.toml`, `crates/api/src/main.rs` (add `fusionpanel-jobs`, `fusionpanel-domain`, `fusionpanel-agent-protocol`)
  * `crates/jobs/` (only if missing helper methods for listing/log append)
  * `crates/control-store/` (only if node registry table/queries are needed)
* **Contract**:
  * `POST /v1/nodes/register` & `POST /v1/nodes/:id/heartbeat` (authenticated via `FUSIONPANEL_ADMIN_TOKEN` / node token)
  * `GET /v1/nodes` (authenticated via admin Bearer OR valid `SESSION_ID` cookie)
  * `POST /v1/jobs` (enqueue job with idempotency key), `GET /v1/jobs`, `GET /v1/jobs/:id`
  * `POST /v1/jobs/claim` (atomic lease claim for agent), `POST /v1/jobs/:id/complete` (or `/logs` + `/fail`)
* **Unit Tests Only**:
  * `RUST_MIN_STACK=33554432 cargo test --offline -p fusionpanel-api --bin fusionpanel-api`
  * `RUST_MIN_STACK=33554432 cargo test --offline -p fusionpanel-jobs --lib`
  * `RUST_MIN_STACK=33554432 cargo test --offline -p fusionpanel-control-store --lib`
  * Keep these target filters on the atomic branch; package-wide test commands may also run integration tests. The root `Cargo.lock` is integration-owned: do not commit a lock change from this branch. After integration updates it for the API dependencies, rerun these unit targets with `--locked` on `sprint-2`.
* **PR Target**: `feat/sprint-2-node-jobs-api` -> `sprint-2`

### Task 2.2 — `feat/sprint-2-agent-job-worker` (`crates/agent` Runner & Heartbeat)
* **Primary Paths**:
  * `crates/agent/Cargo.toml`, `crates/agent/src/`
  * `crates/agent-protocol/` (only if shared request/response types need additions)
* **Contract**:
  * On startup (`FUSIONPANEL_API_URL`, `FUSIONPANEL_ADMIN_TOKEN`, `FUSIONPANEL_NODE_NAME`): register node -> start heartbeat loop -> poll `/v1/jobs/claim` -> execute only the implemented `system.info` and `site.validate` job types (no arbitrary shell execution) -> report output summary + final status (`succeeded`/`failed`).
  * Keep bearer credentials on protected transport: allow plain HTTP only for `localhost` or a loopback IP; require HTTPS for remote API URLs, typically through a TLS-terminating reverse proxy while the API's plain-TCP listener stays loopback-only.
  * Keep `#![forbid(unsafe_code)]`.
* **Unit Tests Only**:
  * `RUST_MIN_STACK=33554432 cargo test --offline --locked --manifest-path crates/agent/Cargo.toml --lib`
  * `cargo check --offline --locked --manifest-path crates/agent/Cargo.toml --bin fusionpanel-agent`
* **PR Target**: `feat/sprint-2-agent-job-worker` -> `sprint-2`

### Task 2.3 — `feat/sprint-2-nodes-jobs-ui` (Frontend Nodes & Jobs Views)
* **Primary Paths**:
  * `frontend/src/api/index.ts`, `frontend/src/types/`
  * `frontend/src/views/Dashboard/`, `frontend/src/views/Nodes/` (or `Administration`), `frontend/src/views/Jobs/`
* **Contract**:
  * Replace static placeholders with live calls to `GET /v1/nodes`, `GET /v1/jobs`, `POST /v1/jobs` (e.g., "Run Ping / System Info Job" button), and `GET /v1/jobs/:id` log viewer.
  * Keep existing auth flow (`ProtectedRoute`, `SESSION_ID` cookie).
* **Unit Tests Only**:
  * `cd frontend && npm test -- --runInBand` (or specific view test file) + `npm run build`
* **PR Target**: `feat/sprint-2-nodes-jobs-ui` -> `sprint-2`

---

## 3. Out of Scope (Do Not Implement in Sprint 2)

* Multi-node WAN clustering, Raft consensus, WireGuard mesh (deferred to post-core Sprint 12).
* Reverse proxy / Nginx / TLS / Database / Mail provisioning jobs (Sprints 3–11).
* PHP runtime management (explicitly deprioritized).

---

## 4. Sprint 2 Close Gate (Run After All Implementation PRs and the Version Bump Are Merged into `sprint-2`)

1. Run full workspace tests + agent crate tests + frontend tests & production build:
   * `RUST_MIN_STACK=33554432 cargo test --locked --workspace`
   * `RUST_MIN_STACK=33554432 cargo test --locked --manifest-path crates/agent/Cargo.toml`
   * `cd frontend && npm test -- --runInBand && npm run build`
2. Run **Live E2E** (no Docker required):
   * `fusionpanel-cli bootstrap operator` -> start `fusionpanel-api` + `fusionpanel-agent` -> browser/cookie login -> verify node appears online via `/v1/nodes` -> enqueue job via `/v1/jobs` -> verify agent executes & persists `succeeded` + logs -> restart both API & agent -> verify persisted node/job history. This final process-restart step has not been repeated on the versioned Sprint 2 head and remains unqualified there.
3. Complete release preparation after the Sprint gate is green:
   * The workspace version bump to `0.3.0-dev.1` is complete in PR #588.
   * Build the package from the versioned `sprint-2` head with `bash packaging/build-package.sh`.
   * Update `docs/execution/SPRINTS.md` with verified results, then open the single release PR **`sprint-2` -> `master`**.

### Verified Sprint 2 Results — 15 November 2026

The implementation, version bump, and evidence PRs are integrated on `sprint-2`. The final versioned gate ran on `72b88a0a55e3c646579dda8f716eeb2daa43321a`; exact PR heads and regular merge commits are recorded in [PR_ISSUE_TRACKER.md](PR_ISSUE_TRACKER.md).

- Version bump: PR #588 changed the Cargo workspace version from `0.2.0-dev.1` to `0.3.0-dev.1`; the lockfile changed only the five workspace packages that inherit that version. Merge commit: `72b88a0a55e3c646579dda8f716eeb2daa43321a`.
- Workspace/integration: `RUST_MIN_STACK=33554432 cargo test --locked --workspace` — **102/102 passed** on the versioned head; local loopback access was enabled for CLI wiremock tests.
- Agent crate: `RUST_MIN_STACK=33554432 cargo test --locked --manifest-path crates/agent/Cargo.toml` — **25 passed** (library and crate integration targets).
- Frontend: `cd frontend && npm test -- --runInBand` — **6 suites / 26 tests passed**; `npm run build` — **passed**.
- Historical live E2E, no Docker: recorded at implementation gate head `55e1a1434a2c6be1fbb64477a7fb39eab242d1d3`. A disposable SQLite operator logged in through the cookie session, registered a local HTTP agent, ran `system.info` to `succeeded`, and manually checked persistence after restarting API and agent. That process-restart check was not repeated on the final versioned head. The final-head Docker/Playwright E2E covers login, a live node heartbeat, and a successful `system.info` job; it does not restart either process.
- Package: `bash packaging/build-package.sh 0.3.0-dev.1 /tmp/fusionpanel-sprint2-final-package` — **passed** on `72b88a0a55e3c646579dda8f716eeb2daa43321a`. Archive: `fusionpanel-0.3.0-dev.1-debian-x86_64.tar.gz`, 5,321,299 bytes; SHA-256 `a1c8dd8982163633fcf8b6c1f729f984e568d4f69c835b7422a0e6291194e3a7`. `BUILD_INFO.txt` records the same source commit.

**Remaining release work:** complete release review and checks on open PR [#590](https://github.com/scherenhaenden/FusionPanel/pull/590), repeat the process-restart E2E on its final head, then obtain product-owner acceptance. No date/hash release tag or published artifact is recorded. Issue [#24](https://github.com/scherenhaenden/FusionPanel/issues/24) remains open. Current E2E does not qualify remote-node transport, clean-host installation, process restarts on the final head, or multi-node operation.

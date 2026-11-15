# Sprint 2 Plan & Handoff Guide — Node Agent & Jobs Runtime

**Sprint Branch**: `sprint-2` (branched from `master` @ `6b5adf5`, `v0.2.0-dev.1`)  
**Target Release**: `sprint-2` -> `master` (`0.3.0-dev.1` upon sprint gate pass)  
**Docker Required**: **No** (runs natively with `fusionpanel-api`, `fusionpanel-agent`, `fusionpanel-cli`, and SQLite)  
**Estimated Effort**: L (8 story points)  

---

## 1. Sprint Goal (100% E2E Deliverable)

Enable the FusionPanel control plane to **register a local node, enqueue jobs via API/UI, claim and execute them on `fusionpanel-agent`, and persist execution status and logs in SQLite across API/agent restarts**.

### What Works at Sprint 2 Close
1. Local `fusionpanel-agent` registers with `fusionpanel-api` and sends periodic heartbeats.
2. Authenticated operators (cookie session or Bearer token) can list nodes, enqueue a job, poll/view job status (`queued` -> `running` -> `succeeded` / `failed`), and inspect job logs from both the REST API and React UI.
3. Restarting `fusionpanel-api` or `fusionpanel-agent` preserves node records, queued/completed jobs, and logs in `FUSIONPANEL_DATABASE_URL`.

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
  * `RUST_MIN_STACK=33554432 cargo test --locked -p fusionpanel-api`
  * (Plus `cargo test --locked -p fusionpanel-jobs` / `-p fusionpanel-control-store` if edited)
* **PR Target**: `feat/sprint-2-node-jobs-api` -> `sprint-2`

### Task 2.2 — `feat/sprint-2-agent-job-worker` (`crates/agent` Runner & Heartbeat)
* **Primary Paths**:
  * `crates/agent/Cargo.toml`, `crates/agent/src/`
  * `crates/agent-protocol/` (only if shared request/response types need additions)
* **Contract**:
  * On startup (`FUSIONPANEL_API_URL`, `FUSIONPANEL_ADMIN_TOKEN`, `FUSIONPANEL_NODE_NAME`): register node -> start heartbeat loop -> poll `/v1/jobs/claim` -> execute supported core job types safely (e.g., `system.ping` / `system.info` / command allowlist, **no** `unsafe_code`, **no** arbitrary shell injection) -> report stdout/stderr summary + final status (`succeeded`/`failed`).
  * Keep `#![forbid(unsafe_code)]`.
* **Unit Tests Only**:
  * `RUST_MIN_STACK=33554432 cargo test --locked --manifest-path crates/agent/Cargo.toml`
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

## 4. Sprint 2 Close Gate (Run ONLY After All 3 PRs Are Merged into `sprint-2`)

1. Run full workspace tests + agent crate tests + frontend tests & production build:
   * `RUST_MIN_STACK=33554432 cargo test --locked --workspace`
   * `RUST_MIN_STACK=33554432 cargo test --locked --manifest-path crates/agent/Cargo.toml`
   * `cd frontend && npm test -- --runInBand && npm run build`
2. Run **Live E2E** (no Docker required):
   * `fusionpanel-cli bootstrap operator` -> start `fusionpanel-api` + `fusionpanel-agent` -> browser/cookie login -> verify node appears online via `/v1/nodes` -> enqueue job via `/v1/jobs` -> verify agent executes & persists `succeeded` + logs -> restart both API & agent -> verify persisted node/job history.
3. Bump workspace version (e.g., `0.3.0-dev.1`), build package with `bash packaging/build-package.sh`, update `docs/execution/SPRINTS.md`, and open the single release PR **`sprint-2` -> `master`**.

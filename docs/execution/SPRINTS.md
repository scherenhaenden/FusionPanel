# FusionPanel — Delivery Sprints & Feature Status

Source of truth for active local development progress. Updated at the close of each sprint.

## Workflow Rules (Mandatory)

```text
master
  └── sprint-<N>                      (sprint integration branch; NO direct feature commits)
        ├── feat/<atomic-feature-1>   ──(unit tests only)──▶ PR into sprint-<N>
        ├── feat/<atomic-feature-2>   ──(unit tests only)──▶ PR into sprint-<N>
        └── fix/<atomic-fix>          ──(unit tests only)──▶ PR into sprint-<N>
              │
              ▼  [All sprint features merged into sprint-<N>]
        Run integration + E2E + quality gates on sprint-<N> (+ fix PRs if needed)
        Update docs/execution/SPRINTS.md & related docs
              │
              ▼
        Single PR: sprint-<N> ──▶ master
```

1. **Sprint Integration Branch (`sprint-<N>`)**: Each sprint starts by creating `sprint-<N>` from `master` (and pushing `origin/sprint-<N>`). Do **not** commit feature changes directly onto `sprint-<N>`.
2. **Atomic Feature Branches (`feat/<topic>`, `fix/<topic>`, `docs/<topic>`) -> `sprint-<N>`**:
   - Every task/feature in a sprint is planned as an atomic branch created from `sprint-<N>`.
   - On each atomic branch, run **only local unit tests** for the touched crate/module/view (avoid running full workspace/E2E suites for every small change).
   - When unit tests pass, open an atomic PR **targeting `sprint-<N>`** (never directly targeting `master`).
3. **Sprint Close, Quality Gates & PR to `master`**: Once all atomic PRs for `sprint-<N>` are merged into `sprint-<N>`:
   - Run full **workspace integration tests + live E2E verification + quality gates** on `sprint-<N>`. If anything fails, land fix PRs into `sprint-<N>` until green.
   - Update this `SPRINTS.md` file (and any related documentation) with exact verification evidence and current feature status.
   - Only when `sprint-<N>` is 100% green and documented, open the single release PR **`sprint-<N>` -> `master`**.
4. **Priority Scope**: Focus on 100% end-to-end usable core features first. PHP management is explicitly **not prioritized**.

---

## Sprint Board

| Sprint | Branch / PR | Scope (100% E2E Goal) | Status | Quality & Verification Gate |
| :--- | :--- | :--- | :--- | :--- |
| **Sprint 1** | `sprint-1` -> `master` | **P0 + P1: Workspace Recovery + Core Control Plane (Auth + Unified API + UI)** | **DONE (100% E2E)** | - CI/module discovery (`python3 -m unittest`): 4/4 OK<br>- Workspace `cargo test --locked --workspace`: all crates passed (`fusionpanel-api` 7/7, `integration-identity` 22/22, `identity` 22/22, `fusionpanel-cli` 11/11, `fusionpanel-agent-protocol` 14/14, `fusionpanel-control-store` 5/5)<br>- Frontend: 5 suites / 17 tests OK + `vite build` OK<br>- Live E2E: CLI operator bootstrap, API + SPA static serving, cookie login, `/v1/auth/me`, `/v1/capabilities`, API restart with SQLite session persistence, logout revocation (`401`) |
| **Sprint 2** | `sprint-2` -> `master` | **P2: Node Agent + Jobs Runtime** (local node registration, SQLite job queue, execution status & logs in API + UI) | **PLANNED** | Pending sprint execution |
| **Sprint 3** | `sprint-3` -> `master` | **P3: Websites / Reverse Proxy + SSL** (`/v1/websites` wired to hosting + jobs + agent + UI, without PHP) | **PLANNED** | Pending sprint execution |
| **Sprint 4** | `sprint-4` -> `master` | **P4: Databases & Backups Core** (`/v1/databases` and `/v1/backups` wired E2E with UI) | **PLANNED** | Pending sprint execution |
| *Backlog* | — | **PHP Management** | **DEPRIORITIZED** | Explicitly excluded per user priority |

---

## What Works Today (Ready to Use)

* **Bootstrap Admin Operator**: `fusionpanel-cli bootstrap operator --database-url <db> --username <user> --password <pass>`
* **Unified Control Plane Server**: `fusionpanel-api` serves:
  * `/v1/health`, `/v1/capabilities` (both `Bearer $FUSIONPANEL_ADMIN_TOKEN` for CLI and `SESSION_ID` cookie for browser)
  * `/v1/auth/login`, `/v1/auth/refresh`, `/v1/auth/logout`, `/v1/auth/me` backed by persistent SQLite (`SqliteIdentityStore` + `ControlStore`)
  * Built React SPA from `FUSIONPANEL_FRONTEND_DIR` (or `./frontend/dist`) on `/` and client routes

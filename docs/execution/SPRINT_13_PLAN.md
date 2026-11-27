# Sprint 13 Plan — Single-Node Containers

- **Status** · Kickoff / Active parallel implementation · 27 November 2026
- **Integration branch** · `sprint-13` from `master` (`a8861e6`)
- **Related references** · [SPRINTS_11_20_PLAN.md](SPRINTS_11_20_PLAN.md) · [S28](sessions/S28.md) · [S40](sessions/S40.md) · [S45](sessions/S45.md) · [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) · [#48](https://github.com/scherenhaenden/FusionPanel/issues/48)

Part of the [Sprints 11–20 delivery plan](SPRINTS_11_20_PLAN.md).

## Goal

An operator can launch an approved digest-pinned container image on a managed node from the panel, inspect live container state and bounded logs, stop and restart the workload, and remove it with clean port release—without affecting other tenants or unmanaged host containers.

## Starting point and kickoff

- `sprint-13` is branched from `master` at `a8861e6` (`a8861e6110efaae7dbc589474335ca98c93a44c2`, Sprint 9 merge, workspace `0.10.0-dev.1`). Sprints 10, 11, and 12 are in flight; Sprint 13 kickoff proceeds in parallel with disjoint write paths. Before Sprint 13 qualification and release PR to `master`, `sprint-13` must rebase/merge the verified `master` containing Sprints 10–12 and record the resulting version bump.
- Open issues [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) and [#48](https://github.com/scherenhaenden/FusionPanel/issues/48) track the container runtime, app catalog, and E2E scope ([S28](sessions/S28.md), [S40](sessions/S40.md), [S45](sessions/S45.md)).
- Three standalone Rust packages already exist in the repository and serve as the foundation to inspect and wire:
  - [`integration/containers/`](../../integration/containers/) (`integration-containers`): Axum `/v1/containers` route handlers, `ContainerStore` trait, `PortAllocator`, and tenant-prefix/store ownership checks.
  - [`modules/containers/`](../../modules/containers/) (`containers`): domain models (`ContainerId`, `ContainerSpec`, `ContainerStatus`, `ImageRef`, `PortMapping`, `MountPoint`), `ContainerEngine` trait, `DockerCliEngine`, `BoundedLogReader` with credential redaction, and resource/health helpers.
  - [`modules/app-catalog/`](../../modules/app-catalog/) (`app-catalog`): `AppTemplate` schema and variable validation, dependency solver, installer, and scanner primitives.
- Confirm availability of a disposable Docker daemon, local OCI registry fixture, and Playwright browser runner before sprint-close qualification.

## Work items and branch ownership

All branches below start from `sprint-13` (`a8861e6`) and all PRs target `sprint-13`. Points are relative planning estimates, including associated fixture work.

| Work item | Branch, worktree, and issue | Prerequisite | Estimate |
| --- | --- | --- | --- |
| Docs · Sprint 13 plan | `docs/sprint-13-plan` (`FusionPanel-s13-docs`) | Kickoff | — |
| 13.1a · Container API and job contract | `feat/sprint-13-container-contract` (`FusionPanel-s13-contract`) · [#30](https://github.com/scherenhaenden/FusionPanel/issues/30), [#48](https://github.com/scherenhaenden/FusionPanel/issues/48) | Kickoff | 3 |
| 13.2 · Container worker and Docker adapter | `feat/sprint-13-container-worker` (`FusionPanel-s13-worker`) · [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) | 13.1a job contract merged | 5 |
| 13.1b · Containers UI | `feat/sprint-13-container-ui` (`FusionPanel-s13-ui`) · [#30](https://github.com/scherenhaenden/FusionPanel/issues/30), [#48](https://github.com/scherenhaenden/FusionPanel/issues/48) | 13.1a DTOs and routes merged | 2 |
| 13.3 · Container E2E fixture | `test/sprint-13-container-e2e` (`FusionPanel-s13-e2e`) · [#30](https://github.com/scherenhaenden/FusionPanel/issues/30), [#48](https://github.com/scherenhaenden/FusionPanel/issues/48) | 13.1b and 13.2 merged | 3 |

### 13.1a · Container API and job contract

**Write paths:** `crates/api/`, `crates/control-store/`, and `crates/agent-protocol/` for container routes, SQLite schema/persistence, RBAC permissions, and typed agent job request/result payloads. The integration owner handles root `Cargo.lock` updates if wiring a reviewed path dependency requires them.

- Add `/v1/containers` (and `/api/v1` mirror if applicable) endpoints for catalog/approved image selection, workload creation (`POST /v1/containers`), inspection (`GET /v1/containers`, `GET /v1/containers/{id}`), bounded log retrieval (`GET /v1/containers/{id}/logs`), lifecycle actions (`start`, `stop`, `restart`), and removal (`DELETE /v1/containers/{id}`).
- Define typed `agent-protocol` request/result variants for container lifecycle and log operations, carrying tenant-owned workload ID, digest-pinned or approved `ImageRef`, bounded CPU/RAM limits, normalized port mappings, and explicit failure states.
- Derive tenant and authorization from the session or scoped API token (`fp_tok_`). Enforce tenant ownership on every read, log fetch, lifecycle action, and delete; return `403` for foreign-tenant access and read-only roles.
- Enforce normalized port collision checks, idempotent request handling (distinguishing exact replay from conflicting replay), and consistent desired-state plus job-reservation persistence in `control-store`. Keep environment secrets out of public DTOs, job logs, and audit details.
- **Unit acceptance:** tenant ownership and foreign-tenant `403`, read-only role denial, normalized port collision rejection, invalid/unpinned image reference rejection, idempotent replay vs. conflict, and honest failure states. Run:
  `CARGO_BUILD_JOBS=2 cargo test --locked --offline -p fusionpanel-api -p fusionpanel-control-store -p fusionpanel-agent-protocol container`

### 13.2 · Container worker and Docker adapter

**Write paths:** `crates/agent/`, `integration/containers/`, and `modules/containers/` only for node-scoped Docker worker execution, label/ownership enforcement, and bounded log/reconciliation support within this slice. Later changes to shared `agent-protocol` contracts are separate coordinated PRs.

- Wire the container job variants in `crates/agent/` using `modules/containers/` (`DockerCliEngine`, `BoundedLogReader`, domain models) and `integration/containers/` (`PortAllocator`, `ContainerStore`).
- Stamp every managed container with explicit FusionPanel ownership and tenant/workload labels so operations (`start`, `stop`, `restart`, `logs`, `remove`) only target panel-managed, labeled containers and never delete or mutate unmanaged host containers.
- Handle container name/port collisions cleanly, support idempotent retry without creating duplicate containers, reconcile container state across agent/daemon restarts, and return bounded, secret-redacted logs and sanitized error results to the control plane.
- Enforce bounded CPU/RAM limits and reject privileged flags or arbitrary host mounts outside the supported single-node slice.
- **Unit acceptance:** apply and error mapping, container ownership label verification, retry without duplicate containers, no unmanaged container deletion, restart reconciliation, and bounded/redacted log output. Run:
  `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path crates/agent/Cargo.toml container`
  `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path modules/containers/Cargo.toml`
  `CARGO_BUILD_JOBS=2 cargo test --locked --offline --manifest-path integration/containers/Cargo.toml`

### 13.1b · Containers UI

**Write paths:** `frontend/src/views/Containers/`, route and sidebar registration, typed frontend API client/types, and focused Jest unit tests (`frontend/src/__tests__/Containers.test.tsx` and affected `api.test.ts` / `routes.test.tsx`).

- Expose `/containers` in the navigation and router for operators with the required container permission.
- Implement catalog/approved image selection, bounded CPU/RAM and port configuration, workload status display, bounded log viewer, lifecycle controls (`Start`, `Stop`, `Restart`), and confirmed removal through the production API.
- Distinguish loading, empty, permission-denied (read-only role), stale/queued job status, and mutation/apply failure states with retry support. Keep sensitive env/config values out of UI history after submission.
- **Unit acceptance:** capability/role gating (read-only role), form validation, lifecycle actions, confirmed delete, stale status and failure handling, and log line limits. From `frontend/`, run:
  `CI=true npm test -- --runInBand src/__tests__/Containers.test.tsx src/__tests__/api.test.ts src/__tests__/routes.test.tsx`

### 13.3 · Container E2E fixture

**Write paths:** `tests/e2e/containers/` for the disposable Docker, local registry, and Playwright browser E2E fixture.

- Build a self-contained, disposable E2E lane under `tests/e2e/containers/` that starts a local OCI registry (or pinned digest fixture), candidate `fusionpanel-api`, `fusionpanel-agent`, and `frontend`, plus an unmanaged sentinel container labeled outside FusionPanel ownership.
- Pin all fixture image digests and container names/labels deterministically, and ensure teardown removes only test-owned, labeled containers, networks, and temporary directories.
- **Unit acceptance:** validate fixture manifest/wiring, pinned inputs, and cleanup label filters without leaving orphaned Docker resources. Run the fixture dry-run/wiring check in `tests/e2e/containers/` before sprint-close E2E execution.

## Sprint integration and acceptance

1. After kickoff, advance `docs/sprint-13-plan` and `feat/sprint-13-container-contract` (13.1a) first. Once 13.1a lands on `sprint-13`, advance `feat/sprint-13-container-worker` (13.2) and `feat/sprint-13-container-ui` (13.1b) against that integration state. Review unit evidence and merge each atomic PR into `sprint-13` with a regular merge commit.
2. Land `test/sprint-13-container-e2e` (13.3) once 13.1b and 13.2 are merged.
3. Rebase/merge verified `master` (carrying Sprints 10–12) into `sprint-13`, apply the sprint version bump (`chore/sprint-13-version-bump`), freeze and record the candidate SHA and `master` base SHA, and run the following gates against that candidate:

| Gate | Required observation |
| --- | --- |
| Workspace and package regression | Locked workspace unit and integration suites (`cargo test --locked --offline --workspace`), standalone `crates/agent`, `modules/containers`, `integration/containers`, and `modules/app-catalog` suites, `frontend` Jest suite and `vite build`, plus `cargo fmt` / `cargo clippy` and required CI checks. |
| Single-node container journey · disposable Docker + registry + browser | Start candidate API, agent, UI, disposable Docker daemon/registry fixture, and an unmanaged sentinel container. In a real browser, launch a digest-pinned image with bounded CPU/RAM and a mapped port, read served content from the running container, inspect bounded logs, stop and restart the workload, and verify port release on remove. Verify port-collision rejection, foreign-tenant `403` isolation, API/agent restart preserving workload identity, and untouched unmanaged sentinel container. |

4. Keep feature checks local and focused. For the final gate, preflight RAM, swap, and disk (at least 10 GiB free at the Cargo target), record Docker storage/volume inventory before and after runs, use bounded timeouts and ephemeral local ports, and remove only attributable fixture resources.
5. Record exact commands, elapsed times, runner/fixture versions, sanitized logs, teardown evidence, and limitations. Update [SPRINTS.md](SPRINTS.md), [PR_ISSUE_TRACKER.md](PR_ISSUE_TRACKER.md), and this plan, then open one release PR `sprint-13` → `master` after all required gates pass.

## Scope limits

Sprint 13 delivers single-node container management for a bounded set of supported options (approved/digest-pinned images, CPU/RAM limits, managed port mappings, and bounded logs). Arbitrary Compose files, privileged workloads, arbitrary host mounts, catalog image CVE/SBOM scanning deploy gates, GPU passthrough, multi-node container scheduling, and immutable revision promotion/rollback (Sprint 14) remain outside this release claim.

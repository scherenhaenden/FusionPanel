# Sprint 3 End-of-day Handoff — 16 November 2026

## Current state

- Integration branch: `sprint-3` at `a92f889a9a9c9afa2a2f722b9d7d59cfad9c87d3`, synchronized with `origin/sprint-3` when this checkpoint was prepared.
- PRs #620–#630 are merged to `sprint-3`; GitHub showed no open PRs targeting `sprint-3` at checkpoint time.
- Issue [#45 — Hosting and runtime E2E](https://github.com/scherenhaenden/FusionPanel/issues/45) remains open.
- Sprint 3 is **not complete or release-qualified**. The website UI/API and agent helper are integrated, but the full browser E2E does not pass. No Sprint 3 version bump, package, or PR to `master` has been prepared.
- PR traceability and atomic scope are recorded in [PR_ISSUE_TRACKER.md](./PR_ISSUE_TRACKER.md); the agreed contract and scope are in [SPRINT_03_PLAN.md](./SPRINT_03_PLAN.md).

## Latest release-gate attempt

Command used from the integration worktree:

```bash
DOCKER_CONFIG=/home/edward/Development/web-panel/.fusionpanel-buildx.gYZvK4/docker-config \
DOCKER_CONTEXT=default RUNNER_TEMP=/tmp \
bash tests/e2e/hosting/fixtures/sprint3/run.sh
```

The run used Docker Engine 29.1.3, Compose 2.40.3, and Buildx 0.30.1 on integration commit `a92f889`. The build and frontend build completed; Compose services became healthy, `nginx -t` passed, and the fixture's deliberately invalid upstream probe was rejected as expected. Playwright authenticated and created the static test site. Its persisted state became `failed` rather than `active`; the test timed out after 90 seconds at `tests/e2e/hosting/sprint3.spec.mjs:177`. The UI showed the safe generic message “Website configuration could not be applied”.

This is the current concrete blocker. The test did **not** reach reverse-proxy traffic, delete, or rollback acceptance. The underlying apply error was not identified in this run: fixture teardown removed its containers and the disposable state. The API/agent error mapping appears to expose only generic safe text, so inspect the job's logs and the `ManagedSiteError` path before deciding whether the failure is in Nginx apply, validation, reload, or state reporting. Treat each as a hypothesis until logs prove it.

The immediate resume sequence is:

1. Check `df -h / /tmp`, `free -h`, and `docker system df` before another Docker run. The fixture enforces its own minimum free-space and available-memory checks and serializes runs with a private user lock.
2. Inspect `GET /v1/jobs/:id/logs` and the agent worker's `ManagedSiteError` mapping. Add enough safe diagnostic detail to the disposable E2E output to expose the actual failure without leaking tokens or credentials.
3. Fix the proven cause in an atomic PR to `sprint-3`; run only the changed unit/preflight checks on that branch.
4. Rerun the full Sprint 3 Docker/Playwright fixture on the integration branch. Continue through static and reverse-proxy HTTP, delete, and invalid-config preservation checks. Do not close #45 or prepare the release PR until this complete gate and the other documented Sprint 3 close gates pass.

Before resuming, verify Buildx is available with `docker buildx version`. The run used a temporary `DOCKER_CONFIG` to provide Buildx; that temporary directory was removed at shutdown. If the default Docker CLI configuration lacks Buildx, recreate only a bounded task-specific plugin/config directory, check disk and Docker usage first, and remove that exact directory after the run.

## Verification already completed

| Scope | Evidence | Boundary |
|---|---|---|
| Websites API (#623) | `cargo fmt --all -- --check`; API tests 16/16; control-store library tests 16/16. | Atomic unit coverage only; does not prove browser/agent/Nginx behavior. |
| Nested agent lock (#625) | Nested agent crate library tests 27/27. | Atomic crate coverage. |
| Sprint integration at `e107a0b` | Full `cargo test --locked --offline --workspace`; nested agent crate 43 tests; frontend 7 suites / 36 tests passed. | Predates later fixture and E2E contract fixes; not a final-head full-suite claim. |
| Current frontend/E2E contract | Frontend production build passed during the latest fixture run; `node --check tests/e2e/hosting/sprint3.spec.mjs` passed for #630. | Build and syntax checks do not qualify website runtime behavior. |
| Fixture and Nginx ownership fixes (#627–#629) | Preflight checks passed (25 for #627; 26 for #628/#629); shell syntax and diff checks passed. | Local fixture checks; the latest live apply still fails. |
| Full Docker/Playwright on `a92f889` | Docker/Compose healthy, Nginx config valid, invalid-upstream probe rejected; site creation failed to reach `active`. | **FAIL / release blocker.** Reverse-proxy, delete, and rollback portions were not reached. |

## Resource and workspace checkpoint

- Before fixture runs, host capacity was checked. The last recorded snapshot showed 216 GB free on `/`, `/tmp` at 362 MB used of 16 GB, and about 15 GiB available memory. Recheck before resuming; these values are not a standing guarantee.
- The fixture cleaned its own containers, networks, volumes, image tags, Buildx builder, and temporary state. No global Docker prune was used; unrelated containers were left running.
- Docker usage returned to the recorded baseline: 26 images / 31.52 GB, 16 containers (4 active), 3 volumes / 1.874 GB, and 1.901 GB build cache. The temporary 353 MB BuildKit image pulled by the run was removed only after checking no container used it. Verify current usage rather than assuming this snapshot is still current.
- The existing Cargo `target/` directory is intentionally retained for continuation. The unrelated `/home/edward/Development/web-panel/FusionPanel-versioning` worktree is not part of Sprint 3 and must be preserved.
- Readiness catalog percentages have not been recalculated. The linked 162-feature snapshot is the historical 10 November snapshot, not a current post-Sprint-3 acceptance count: [READINESS_CATALOG_2026-11-10.md](./READINESS_CATALOG_2026-11-10.md) and [READINESS_SNAPSHOT_2026-11-10.md](./READINESS_SNAPSHOT_2026-11-10.md).

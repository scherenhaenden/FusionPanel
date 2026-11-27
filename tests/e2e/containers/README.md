# Sprint 13 Container E2E Fixture

This directory contains the Tier-2 (disposable Docker/Compose) end-to-end test fixture and Playwright browser/API acceptance suite for Sprint 13 container management (`CTR-01`, `CTR-04`, `CTR-05`, `CTR-11` isolation/ownership).

## Contents

- [`docker-compose.yml`](./docker-compose.yml): Disposable stack with a local Docker registry (`registry:2.8.3`), an unmanaged sentinel container (`sentinel`), `fusionpanel-api` (`api`), and `fusionpanel-agent` (`agent`).
- [`Dockerfile.agent`](./Dockerfile.agent) & [`start-agent.sh`](./start-agent.sh): Agent container image and entrypoint that waits for `api` health and runs `fusionpanel-agent` against the mounted Docker socket.
- [`fixtures/test-app/Dockerfile`](./fixtures/test-app/Dockerfile) & [`fixtures/test-app/index.html`](./fixtures/test-app/index.html): Minimal digest-pinned HTTP test image (`busybox httpd`) pushed to the disposable local registry at runtime.
- [`run.sh`](./run.sh): End-to-end orchestrator that checks Docker disk space, builds binaries/frontend, starts the disposable stack, pushes the test image to obtain an immutable `@sha256:` digest, runs Playwright, and tears down all task-owned resources on exit.
- [`playwright.sprint13.config.mjs`](./playwright.sprint13.config.mjs): Playwright configuration (single worker, Chromium).
- [`sprint13.spec.mjs`](./sprint13.spec.mjs): Browser and API E2E test suite.

## Covered Scenarios

1. **Digest-pinned launch**: Launches a container workload from the local registry using its immutable `@sha256:` digest.
2. **Real HTTP content verification**: Reads HTTP content directly from the container's published host port.
3. **Lifecycle & port release**: Stops, restarts, and removes the container, verifying HTTP failure while stopped, recovery on restart, and host port release after removal.
4. **Collision & foreign-tenant rejection**: Verifies duplicate name/port collisions return `409 Conflict` and a foreign-tenant operator receives `403`/`404` when attempting to read or mutate another tenant's container.
5. **API & agent restart identity preservation**: Restarts `api` and `agent` via `docker compose restart` and verifies the workload ID, Docker container ID, digest, and `running` state persist without duplicate containers.
6. **Unmanaged sentinel preservation**: Confirms the unmanaged `sentinel` container (which has no `fusionpanel.*` labels) remains running with its original container ID and HTTP response throughout lifecycle operations and cleanup.

## Current Status

- **Status**: `BLOCKED` until Sprint 13 container API, agent driver, and frontend UI branches are integrated onto `sprint-13`.
- **Exclusive write path**: `tests/e2e/containers/`

## How to Run (After `sprint-13` Integration)

1. Ensure Docker daemon, Node.js, Playwright (`tests/e2e/browser/node_modules`), Rust toolchain, and at least 10 GiB free disk space are available.
2. Run:
   ```bash
   bash tests/e2e/containers/run.sh
   ```
3. Logs and machine-readable evidence are written to `/tmp/fusionpanel-sprint13-e2e-<suffix>.log` and `/tmp/fusionpanel-sprint13-e2e-<suffix>-evidence.json`.

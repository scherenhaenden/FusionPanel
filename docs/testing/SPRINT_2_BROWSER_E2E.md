# Sprint 2 browser E2E fixture

The full Sprint-to-master and master CI gate runs the Nodes/Jobs browser flow against the actual FusionPanel API, frontend, and agent. The test seeds a synthetic operator into a fresh SQLite database, signs in through the real login endpoint and session cookie, confirms the agent heartbeat in the Nodes view, and queues a `system.info` job through the Jobs view. It checks the persisted successful state and agent log in the browser.

The Docker Compose services use Linux host networking. The API binds only to `127.0.0.1`; the agent calls `http://127.0.0.1:<port>` from the shared Linux loopback namespace, and Playwright opens that same loopback origin. This keeps the only plaintext HTTP endpoint on loopback and avoids sending HTTP to a Compose service hostname. No host ports are published to a LAN interface.

## Prerequisites

- Linux x86_64 with a local rootful Docker daemon and Docker Compose v2. Host networking is required; this fixture is intended for the Ubuntu GitHub runner and does not support remote Docker daemons or Docker Desktop networking on macOS or Windows.
- Rust stable, Node.js 20, npm, and Playwright Chromium dependencies.
- The default loopback port `18080` must be unused. Override it with `FUSIONPANEL_E2E_PORT` if needed.
- Build the actual binaries and static frontend first:

  ```sh
  cargo build --locked --manifest-path Cargo.toml -p fusionpanel-api -p fusionpanel-cli
  cargo build --locked --manifest-path crates/agent/Cargo.toml --bin fusionpanel-agent
  (cd frontend && npm ci && npm run build)
  (cd tests/e2e/browser && npm ci && npx playwright install --with-deps chromium)
  ```

Then run `tests/e2e/browser/fixtures/sprint2/run.sh` from the repository checkout. The script creates a unique Compose project/image and temporary SQLite directory, seeds the test operator, runs only `tests/nodes-jobs.spec.js`, captures redacted Compose logs, and removes the containers, image, and temporary database on exit. The runtime uses the Ubuntu 24.04 amd64 image pinned by digest.

## Scope and limits

- This is real browser-to-API-to-agent coverage for the Sprint 2 Nodes/Jobs workflow. It uses no mocked HTTP responses. SQLite and credentials are disposable synthetic data.
- It covers the `system.info` operation only. It does not qualify other agent operations, multi-node behavior, Websites, databases, backups, variants, host installation, or production TLS/network configuration.
- The future `variants.spec.js` remains discovered and compiled by Playwright but is not executed by the Sprint 2 full gate. It still requires its separate disposable site fixture and functionality from a later sprint.
- The full-gate lane uploads the Playwright report, traces/results, and redacted Compose logs for three days. A test failure or a missing Docker prerequisite fails the gate; the lane does not silently skip unavailable dependencies.

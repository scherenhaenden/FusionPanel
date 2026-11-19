# Observability integration and live API E2E

This standalone package contains two different test lanes:

- `tests/e2e.rs` exercises the separate `integration/observability` sample service. It is not the FusionPanel product API.
- `tests/live_api_alerts.rs` starts the real `fusionpanel-api` and `fusionpanel-agent` binaries, uses an isolated temporary SQLite database, and sends webhooks to a receiver bound only to `127.0.0.1`.

The live API lane creates ephemeral credentials at runtime, clears the child processes' inherited environment, and suppresses their output. It reads host telemetry through the agent collector; the agent has no queued jobs and cannot modify host services. It verifies node-scoped ingestion, operator authorization, alert transitions, signed webhook delivery, and persistence of the alert rule, delivery history, and encrypted webhook secret across an API restart. Metrics history is in memory and is expected to be empty after restart.

## Run the live API lane

From the repository root, first build the actual product binaries with bounded local concurrency:

```sh
CARGO_BUILD_JOBS=2 cargo build --locked --offline --manifest-path Cargo.toml --package fusionpanel-api --bin fusionpanel-api
CARGO_BUILD_JOBS=2 cargo build --locked --offline --manifest-path crates/agent/Cargo.toml --bin fusionpanel-agent
```

Then run the live process test against those exact binaries:

```sh
FUSIONPANEL_API_BINARY="$PWD/target/debug/fusionpanel-api" \
FUSIONPANEL_AGENT_BINARY="$PWD/crates/agent/target/debug/fusionpanel-agent" \
CARGO_BUILD_JOBS=2 cargo test --offline --manifest-path tests/e2e/observability/Cargo.toml --test live_api_alerts -- --nocapture
```

The test chooses ephemeral loopback ports and removes the temporary database when it ends. It does not require Docker or an externally running API. Missing product binaries are reported as `BLOCKED`; the test does not silently fall back to a mock server.

## Acceptance limit

The recovery event is produced by a synthetic low-metric sample after the real agent has been stopped. This verifies the product's threshold state transition and webhook behavior, but it does not simulate or qualify a real service outage, uptime monitor, or measured service recovery. S48's real outage-to-alert-to-recovery acceptance remains blocked until FusionPanel exposes an actual outage signal and the end-to-end fixture can observe it.

# Observability E2E Tests

These tests start the real observability router and collector worker on loopback, use disposable HTTP target and webhook fixtures, and store incidents in a temporary SQLite database. They verify authenticated incident reads, metrics, alert delivery, outage recovery, and incident persistence across a service restart.

Run with `cargo test --manifest-path tests/e2e/observability/Cargo.toml`.

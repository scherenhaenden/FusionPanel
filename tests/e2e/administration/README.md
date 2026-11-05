# Administration E2E Tests

The suite starts the real administration router on loopback with a disposable audit journal and invitation journal. It verifies authenticated audit append and tamper detection after restart, invitation acceptance and revocation across restarts, and concurrent quota reservations through HTTP. Updater checks exercise the production updater against temporary files. No shared service, host firewall, or production credential is used.

Run with `cargo test --manifest-path tests/e2e/administration/Cargo.toml`.

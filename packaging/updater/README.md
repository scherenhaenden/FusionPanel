# Updater

In-place self-updater & rollback guard.

## APIs
- `Updater`: struct to perform updates.
- `UpdateConfig`: configuration for updater.

## Tests
- Run tests via `cargo test --manifest-path packaging/updater/Cargo.toml`

## Modes
- Updates the binary in place.
- Verifies signatures before replacing.
- Checks health and rolls back if failing.

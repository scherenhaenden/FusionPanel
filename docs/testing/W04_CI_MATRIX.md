# CI Qualification Matrix

## Discovery Lane
- Run `scripts/ci/discover.py` to identify all `Cargo.toml` and `package.json` manifests.
- Detect dropped files and orphan tests.
- Fail on zero-test success.

## Dependency Lane
- Enforce one authoritative dependency lock strategy (e.g., top-level `Cargo.lock` and `package-lock.json`).

## Compile + Unit Lane
- Matrix across all discovered manifests.
- For each manifest, run compile (e.g., `cargo build` or `npm run build`).
- For each manifest, run tests (e.g., `cargo test` or `npm test`).
- Ensure no zero-test success condition exists.
- Reports PASS/FAIL/BLOCKED/NOT_RUN.

## Stale Base Merge Detection
- Verify the PR base is up-to-date with master.

## Logging and Reporting
- Capture sanitized logs.
- Never silently skip a required release lane.

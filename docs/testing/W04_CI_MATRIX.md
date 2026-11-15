# CI Qualification Matrix

## Discovery Lane
- Run `scripts/ci/discover.py` to identify all `Cargo.toml` and `package.json` manifests.
- Detect dropped files and orphan tests.
- Fail on zero-test success.

## Dependency Lane
- Enforce one authoritative dependency lock strategy (e.g., top-level `Cargo.lock` and `package-lock.json`).

## Compile + Unit Lane
- Matrix across all discovered manifests. Cargo manifests use their Rust build and unit-test checks.
- NPM manifests default to the `unit` policy: run `npm run build` and `npm test`.
- An NPM manifest may declare `"fusionpanel": { "policy": "compile-only" }` when its scripts describe end-to-end tests that must not execute in an atomic branch check. It must provide `npm run compile`, which lists the test cases and fails if none are discovered; this lane does not execute those tests.
- Execute browser/integration suites only on the Sprint integration branch or `master` release gate, where their required services and broader environment are available.
- Ensure no zero-test success condition exists in either policy.
- Reports PASS/FAIL/BLOCKED/NOT_RUN.

## Stale Base Merge Detection
- Verify the PR base is up-to-date with master.

## Logging and Reporting
- Capture sanitized logs.
- Never silently skip a required release lane.

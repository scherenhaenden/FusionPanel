# CI Qualification Matrix

## Pull Request Gate
- On a PR to `master`, run `scripts/ci/discover.py --changed-from <base-sha> --changed-to <head-sha>`.
- Select only the nearest owning package for changed files. Changes inside a root Cargo workspace member select the root workspace manifest so its shared tests run once; standalone Cargo workspaces and npm packages keep their own manifests.
- Docs and workflow-only changes do not fan out across unrelated package manifests. The orphan-test and dropped-file checks still run.
- Cargo package jobs run build, formatting, Clippy, and tests. Npm package jobs run their configured build and unit policy. Empty changed-package lists skip only that package lane; validation still runs.
- Keep job timeouts bounded and cap manifest concurrency. Cache Cargo registries and Git sources, not per-manifest `target` directories. Upload package logs only on failure.
- Cancel an older in-progress PR run when a new commit arrives for the same PR.

## Dependency Lane
- Enforce the top-level Cargo lock and check package-lock files only for the npm packages selected by discovery.

## Full Catalog Audit
- Run `scripts/ci/discover.py` without changed-SHA arguments to enumerate all Cargo and npm manifests.
- The full catalog matrix is an explicit `workflow_dispatch` audit, not an automatic per-release fan-out. It may expose unrelated future-sprint failures; report those as findings rather than claiming they are passed.
- Cargo manifests use their Rust build and unit-test checks.
- NPM manifests default to the `unit` policy: run `npm run build` and `npm test`.
- An NPM manifest may declare `"fusionpanel": { "policy": "compile-only" }` when its scripts describe end-to-end tests that must not execute in an atomic branch check. It must provide `npm run compile`, which lists the test cases and fails if none are discovered; this lane does not execute those tests.
- Execute browser/integration suites only on the final Sprint release PR to `master`, where their required services and broader environment are available. Sprint 2's browser E2E runs only for `sprint-2` -> `master`.
- Ensure no zero-test success condition exists in either policy.
- Reports PASS/FAIL/BLOCKED/NOT_RUN.

## Stale Base Merge Detection
- Verify the PR base is up-to-date with master.

## Packaging
- Build the Debian package once for a PR targeting `master`; atomic PRs targeting `sprint-<N>` use their local unit tests and do not rebuild the package.
- Do not repeat the full qualification matrix on the post-merge `master` push. The release PR is the reviewed gate; full catalog audits are manual.

## Local Runs and Act
- `scripts/ci/run_ci_matrix.sh` requires explicit base and head refs and tests only the affected manifests. It checks for at least 10 GiB free on the filesystem before compiling.
- Use Act for focused workflow wiring checks, not the full dynamic catalog matrix. Act expands each matrix entry into a separate container job and may retain successful-job volumes; inspect Docker storage before and after a run and remove only volumes created by that run.
- Prefer the hosted final Sprint PR gate for the real Docker/browser flow. Do not launch another full local matrix to reproduce unchanged future-sprint failures.

## Logging and Reporting
- Capture sanitized logs.
- Never silently skip a required release lane.

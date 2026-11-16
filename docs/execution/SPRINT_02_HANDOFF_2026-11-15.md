# Sprint 2 Handoff — 2026-11-16 refresh

## Current state

- PR [#602](https://github.com/scherenhaenden/FusionPanel/pull/602) is merged into `sprint-2`, merge commit `67eae339833c0da8adf116d57989a4f0fe75ae6e`. Its package passed in run [43757289385](https://github.com/scherenhaenden/FusionPanel/actions/runs/43757289385). Agent library unit tests passed 16/16; the locked agent binary check, installer environment test, systemd verification, formatting, shell syntax, and diff checks passed. Independent source review found no blocker.
- PR [#590](https://github.com/scherenhaenden/FusionPanel/pull/590) remains open from `sprint-2` to `master`, head `67eae339833c0da8adf116d57989a4f0fe75ae6e`, status `UNSTABLE`.
  - Package run [43758110106](https://github.com/scherenhaenden/FusionPanel/actions/runs/43758110106): PASS.
  - Docker Browser E2E job [132946572841](https://github.com/scherenhaenden/FusionPanel/actions/runs/43758110127/job/132946572841): PASS.
  - CI Qualification run [43758110127](https://github.com/scherenhaenden/FusionPanel/actions/runs/43758110127): FAILURE. `crates/agent` compile, formatting, Clippy, and its 16 library unit tests passed; its separate integration test `test_agent_child_process_integration` failed at `crates/agent/tests/integration.rs:86` because the expected connection error was not logged before the test killed the child.
  - The failing integration test starts a nested `cargo run`, sleeps a fixed 1.5 seconds, then kills the child. The same manifest passed on the previous Sprint 2 run and the master baseline, so this appears timing-sensitive and was exposed by the changed build/dependency timing. It must be fixed and rerun; do not treat this as a proven runtime TLS defect.
  - The broad Cargo matrix still has the known baseline failures. The `tests/e2e/hosting` job was CANCELLED, not passed.
- The Docker Browser E2E proves local node registration, UI job creation, successful `system.info`, and visible logs. It does **not** restart API and agent. Sprint 2 DoD still requires verifying that the succeeded job and logs remain visible after both processes restart.
- PR [#585](https://github.com/scherenhaenden/FusionPanel/pull/585) is still open to `sprint-3`; its package check failed on the old safe-directory setup. Revalidate it after Sprint 2 lands in master.

## Remaining Sprint 2 work

1. Replace the flaky nested `cargo run`/fixed-delay integration-test harness with the prebuilt agent binary and a bounded wait for the expected log. On the atomic branch, run only agent library unit tests plus formatting/syntax checks; run integration tests only after merging into `sprint-2`.
2. Extend the disposable Docker/Playwright fixture: execute a real job, restart API and agent, reopen Nodes/Jobs, and verify the same node is online and the succeeded job/logs persisted. Run Browser E2E only on the integration branch after the atomic PR is merged.
3. Merge those atomic PRs into `sprint-2`, rerun the final Sprint 2 gate, compare broad failures against master, and update sprint evidence. Do not merge #590 until the Sprint 2 relevant gate is complete and failures are explained.
4. Keep issue #24 open until the release PR is merged to master and the required acceptance/evidence is recorded.

## Development package limitation

The current development tarball contains the API and agent, but omits `fusionpanel-cli` and `frontend/dist`; a clean install cannot run the planned CLI bootstrap or serve the UI. The Browser E2E builds CLI/frontend from the checkout and does not qualify this tarball. `packaging/update.sh` also uses a different version-directory layout from `install.sh`. Do not call this an install-ready full product. If that is required for Sprint 2, add an atomic packaging PR to bundle CLI/UI, configure the installed API's frontend path, align updater/rollback, and verify the real artifact in a disposable clean-install Docker smoke test.

## Sprint 3 handoff

After #590 is accepted and merged to `master`, sync `sprint-3` from the new master, rerun and review #585, then merge it and remove its task branch/worktree.

## Get this handoff on another PC

This document is on the published branch `docs/sprint-2-handoff` until its docs PR is merged. In an existing clone:

```sh
git fetch origin
git switch --track origin/docs/sprint-2-handoff
```

After the docs PR is merged into `sprint-2`, switch to `origin/sprint-2` instead.

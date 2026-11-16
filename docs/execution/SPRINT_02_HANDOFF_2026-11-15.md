# Sprint 2 Handoff — refreshed 2026-11-16

## Release snapshot

- Final integrated Sprint 2 head: `2e2e8c719dc4cfba488c6dea4c3b635e16aa6656` on `sprint-2`.
- Atomic PRs #602–#610 are merged into `sprint-2`; the remote has no remaining Sprint 2 fix/docs branches.
- Release PR [#590](https://github.com/scherenhaenden/FusionPanel/pull/590) remains open to `master`, approved, and `UNSTABLE`. Its latest head is the final Sprint 2 commit above. Do not describe the release as merged or accepted.
- Issue [#24](https://github.com/scherenhaenden/FusionPanel/issues/24) remains open. There is no date/hash release tag, published release artifact, or product-owner acceptance.

## Final-head verification

| Gate | Result | Evidence and limits |
|---|---|---|
| GitHub package workflow | PASS | Run [43826668194](https://github.com/scherenhaenden/FusionPanel/actions/runs/43826668194) at the final head. Installer permissions, package layout, updater rollback, URL checks, and package build passed. |
| Clean-install package smoke | PASS | Tested the actual run 43826668194 artifact in disposable Ubuntu 24.04 Docker with networking disabled and read-only source/artifact mounts. SHA-256: `5b73733014b29d63a8618bcf1345e0c444765ac58c4d84a8d666612d0628929f`. Installed CLI/API/agent/UI worked; API health and CLI health passed; UI root and bundled JS returned HTTP 200; the agent stayed active and the generated token was root-owned mode 0600. systemd reload/activation was not tested. |
| GitHub browser E2E | PASS | Job [133162021961](https://github.com/scherenhaenden/FusionPanel/actions/runs/43826668270/job/133162021961) passed. |
| Local act browser E2E | PASS | `act` ran the Docker fixture and Playwright passed 1/1 test. It logged in, ran `system.info`, restarted API and agent, then verified the same node ID, operator session, succeeded job, and logs. A free host port and serial Compose builds were used. |
| Local act validation | PASS | Audit, bypass checks, fixture preflight, seed-admin restart contract, and 4/4 fixture-context unit tests passed. |
| Local act Cargo root | PASS | `Cargo.toml` compile, format, Clippy, tests, and zero-test enforcement passed. |
| Local act agent | PASS | `crates/agent/Cargo.toml`: 16 library tests and 5 integration tests passed, including the child-process test. |
| Local act jobs | PASS | `integration/jobs/Cargo.toml`: 9 tests passed (7 integration and 2 worker tests). |
| Local act frontend | PASS | `frontend/package.json`: 6 suites / 26 tests passed and build passed. |
| Local act browser package | PASS | `tests/e2e/browser/package.json` CI job passed; the separate Docker Playwright E2E also passed. |
| Local act nodes integration | BLOCKED | 5 tests passed. `test_blocked_migration` reports `RESULT=BLOCKED; missing full-machine disposable VMs`; no VM-backed migration evidence is claimed. The GitHub matrix job has the same blocker. |
| Local act package workflow | NOT RUN TO PRODUCT STEPS | The workflow's `debian:bookworm` job image has no Node executable, so act cannot launch `actions/setup-node@v4`. GitHub package run 43826668194 and the real Docker artifact smoke above passed. |
| Full GitHub qualification matrix | NOT GREEN | Run [43826668270](https://github.com/scherenhaenden/FusionPanel/actions/runs/43826668270) ended cancelled after 31 successful jobs and 43 failed jobs; the unrelated `tests/e2e/hosting` job was cancelled to save runner time. All 43 failed job names are in the 46-job master baseline failure set [43707984101](https://github.com/scherenhaenden/FusionPanel/actions/runs/43707984101); no new failed job names were found, and three baseline failures are green on this head. This does not make the overall gate green. |

The browser E2E now covers process-restart persistence on the final head. The clean-install smoke covers the package's CLI, API, agent and UI. Neither test activates systemd or qualifies remote-node transport or multi-node operation.

## Release decision and next steps

1. Keep PR #590 open while its qualification run is `UNSTABLE`; review the inherited matrix failures and the VM-backed migration blocker before release merge.
2. Once the release gate is accepted and PR #590 is merged, close issue #24 only after recording the release artifact and product-owner acceptance.
3. Sprint 3 handoff PR [#585](https://github.com/scherenhaenden/FusionPanel/pull/585) still targets `sprint-3`; revalidate its package workflow after syncing that branch from the accepted Sprint 2 release.

## Continue on another PC

Fetch the merged integration branch and switch to it:

```sh
git fetch origin
git switch --track origin/sprint-2
```

If the clone already has a local `sprint-2`, use `git switch sprint-2` instead.

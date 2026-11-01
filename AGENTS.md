# Development instructions

All documentation, code comments, issues and pull requests are in English. Follow the exclusive write paths in your assigned task. Do not change sibling packages, root manifests, root Cargo.lock, or shared documentation in the parallel foundation batch. Dependencies may be added only to your own Cargo.toml. Do not commit Cargo.lock in this batch; integration owns it.

Read docs/FOUNDATION_CONTRACTS.md before implementation. Use safe Rust and forbid unsafe code. Never log credentials or return fabricated success for unsupported functionality. Do not execute privileged commands or contact production services. Add focused tests of meaningful behavior. Create a separate branch and PR against master; do not merge.

Write original implementation for the specified contracts. Preserve required license and copyright notices for any third-party material you introduce.

## Execution-window rules

The canonical branch is master. Read docs/execution/DELIVERY_PLAN.md and your task spec. Independent modules may edit only their assigned module and must use a standalone `[workspace]` until integration. The root-manifest prohibition applies except when your task explicitly assigns those files. Never remove a capability or user-visible mode to simplify implementation. Follow docs/engineering/FEATURE_PRESERVATION.md and docs/engineering/VARIANT_DESIGN.md. Read docs/testing/TEST_STRATEGY.md; no mocked success may qualify a real dependency or E2E journey. Missing prerequisites are BLOCKED, never PASS. Record actual tests, limits and unresolved gaps. Preserve required notices for reused material.

## Merge and issue policy

Merge approved pull requests with a regular merge commit; preserve their reviewed commit history and never squash. Before merging, refresh the exact PR head, approvals, required checks and master mergeability; use a head-SHA guard. Do not bypass blocked review/version checks or use administrator override. Close an issue only after verifying its linked PR is MERGED on master and recording the merge SHA, user-visible changes, actual unit/integration/E2E commands and results, and remaining limitations in the issue. If no merge occurred, the issue stays open.

Package implementation PRs must link their primary issue from the [PR and issue tracker](docs/execution/PR_ISSUE_TRACKER.md). Non-package PRs may link a directly related issue when one exists; do not invent an unrelated association, and record the reason in the tracker if no relevant issue exists. Use `Refs #N` while any acceptance, review or evidence gate remains incomplete. Use `Fixes #N` only when acceptance is complete and final test evidence is recorded before merge, since GitHub can close a linked issue automatically.

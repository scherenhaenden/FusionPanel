# Coordination and Session Runbook

## Before dispatch

Confirm default branch is master, verify its remote SHA, check session/issue duplicates and prerequisite artifacts, reserve exclusive paths and prepare a prompt with objective, behavior/variant contract, tests, delivery and forbidden actions. Create/update an issue; save the full prompt in the task spec or dispatch artifact. New sessions must branch from the verified SHA, not an assumed recent local checkout.

## Dispatch and monitor

Use the authorized outside-sandbox CLI. Record task ID, issue, base SHA, session ID/URL, status and PR. Listing status is read-only. Do not treat Completed as a merged feature. If creation returns an ambiguous error, inspect the session list before retrying. Quota/concurrency failures leave a queued task with a recorded error. No duplicate sessions to ask for a missing PR.

## Recover a completed patch

Retrieve with `jules remote pull --session ID` without apply. Inspect changed paths and expected base. Create an isolated worktree/branch, apply only that session, run its focused checks, document defects and create a PR against master. Keep each session isolated. A missing PR is a delivery defect, not permission to silently mix results.

## Integration gate

Review immutable PR head, diff, dependency/provenance changes, test evidence, scope and mergeability. Retarget legacy main PRs to master. Merge only with owner authorization under current policy. Never auto-approve agent changes or bypass required checks. Refresh all integrated interfaces before dispatching dependents. Root Cargo.lock and shared contract changes have one integrator at a time.

## Daily report and deadline protection

Maintain accepted/implementing/blocked session counts, critical path, capability/mode gaps and next ready work. Avoid repetitive unchanged reports. Preserve completed patches, branch SHAs and test artifacts before the Nov 10 candidate cutoff. Provider access cutoff on Nov 11 is not assumed to be midnight; finish important remote work early. No recurring automation is created by this document.

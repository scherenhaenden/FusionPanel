# Wave W02 — Audit repairs and foundation integration

**Repository:** `scherenhaenden/FusionPanel`  
**Base:** `master` at `3a8bf9ef39c35a43b36771c575dbc3b11bc601ba`  
**Prepared:** 2026-11-01  
**Coordinator:** local Codex session  
**Scope:** static-source audit, narrowly owned Jules implementation tasks, honest test evidence, and exact issue/PR traceability.

## Outcome of dispatch

Six sessions were dispatched before the coordinator received the user's updated instruction to start the first five and then answer sessions awaiting feedback. The sixth dispatch had already completed at that point. No seventh or later session was dispatched. The six created sessions are left intact; this record does not assume that a dispatched task is currently executing.

| Slot | Package | Issue | Jules session | Exclusive paths | State at dispatch |
|---|---|---:|---|---|---|
| W02-01 | S06 | #6 | [7302772204230979493](https://jules.google.com/session/7302772204230979493) | Root workspace, crates, foundation tests and CI | Dispatched |
| W02-02 | S09 | #91 | [7764619453540187587](https://jules.google.com/session/7764619453540187587) | `modules/hosting/` | Dispatched |
| W02-03 | S10 | #92 | [1885562045966245180](https://jules.google.com/session/1885562045966245180) | `modules/databases/` | Dispatched |
| W02-04 | S11 | #93 | [1116061198015020496](https://jules.google.com/session/1116061198015020496) | `modules/backups/` | Dispatched |
| W02-05 | S13 | #94 | [3274795259823328621](https://jules.google.com/session/3274795259823328621) | `modules/dns/` | Dispatched |
| W02-06 | S15 | #95 | [14127040891990959948](https://jules.google.com/session/14127040891990959948) | `modules/security/` | Dispatched just before updated instruction arrived |
| W02-07 | S16 | #96 | Not created | `modules/observability/` | Queued |
| W02-08 | S18 | #97 | Not created; continue existing PR #69 | `modules/migration/` | Queued |
| W02-09 | S08 | #11 | Not created; continue recovery PR #90 | `modules/identity/` | Queued |
| W02-10 | S23 | #25 | Not created; continue recovery PR #98 | `modules/secrets/` | Queued |

The user reported six other Jules sessions waiting for feedback. This coordinator checked `jules --help`, `jules remote --help`, `jules remote list --help`, `jules remote pull --help`, and `jules teleport --help`. The installed CLI supports session creation, listing, result retrieval, and teleport; it exposes no command for reading the pending conversation turn or sending a response. The browser is disallowed by the Jules workflow instructions. No blind response was sent. To answer accurately, obtain the actual six questions through a supported Jules reply surface or have the user provide their text. The six are recorded in the prior inventory at `/tmp/fusionpanel-current-inventory/sessions.json`; inventory is a capture, not a live state check.

## Concurrency boundary

The last successful dispatch admission record showed eight executing/planning sessions before W02-06, and W02-06 was admitted as the sixth new session. The prior inventory had three FusionPanel sessions in progress. This puts the observed active count at nine after W02-06 if all remained active. The user-reported six waiting-for-feedback sessions plus these nine would reach the requested ceiling of fifteen. A fresh status refresh did not return usable output in this environment, so this is an admission-time count, not a claim about current state. No more sessions are to be launched in this round.

## Audit and test evidence

See [source audit](../../../audits/2026-11-01/SOURCE_AUDIT.md) for source anchors, severity, remediation, and evidence limits. Highest priority findings include unsafe generated hosting configuration and shared PHP-FPM identity, database apply/restore error propagation, backup symlink and rollback safety, DNS regression loss, firewall recovery-access/CSP parsing defects, and missing observability address fallback/webhook payload assertions.

Executed evidence recorded in the source audit:

- `cargo fmt --all -- --check`: passed.
- Root `cargo test --workspace --offline --locked`: blocked because the tracked root lockfile requires update.
- Disposable-copy foundation workspace: 49 tests passed outside the sandbox after offline lock resolution (protocol 14, API 5, CLI 11, domain 10, jobs 9).
- Recovered secrets PR #98: eight crypto/filesystem tests passed; one false-green E2E placeholder was excluded.
- The audit's hosting, database, backup, DNS, security, and observability module test commands were not rerun in this round. Static findings are not runtime acceptance.

The refreshed readiness snapshot remains 3.70% of 162 feature capabilities and 31.43% of 63 packages, with 0/162 fully accepted and 3/63 packages at 100%. The source audit and these sessions do not increase those figures until evidence is reviewed and the ledger is deliberately updated.

## Recovery PRs found

- Draft [PR #90](https://github.com/scherenhaenden/FusionPanel/pull/90), identity recovery, head `ff4caef9a7eba8f9f60b8e570e182a92fdefd325`: compilation only, zero tests. Keep draft pending principal-scope, TTL, persistence, and isolation tests.
- Draft [PR #98](https://github.com/scherenhaenden/FusionPanel/pull/98), secrets recovery, head `18c65b8e5a1e84c03a34791540d2dbd2acf9e034`: eight crypto/filesystem tests passed; unresolved disclosure, identifier, authorization, and atomic rotation/recovery defects block merge.

Existing PR #69 remains the migration delivery target. W02-08 must add commits to that branch and preserve history rather than duplicate it.

## Issue and prompt map

New audit repair issues #91–#97 are linked from [PR/issue tracker](../../PR_ISSUE_TRACKER.md). Full exact prompts and task metadata live beside this README. Each implementation prompt names its base, exclusive write paths, required negative and integration evidence, dependency-block behavior, no-merge rule, and issue association. W02-08 through W02-10 update existing PRs rather than opening competing recovery PRs.

## Follow-up order

1. Obtain and answer the six pending Jules questions through an authorized session-reply surface; if unavailable, have the user provide the prompts.
2. Refresh actual Jules status before dispatching any queued work. Keep no more than fifteen active/planning sessions, counting waiting sessions conservatively.
3. Recover each result without applying it to the shared checkout; review exact paths and base, run meaningful scoped tests, then publish one focused PR.
4. Merge only reviewed heads with required checks green, using regular merge commits. Update issue dispositions and the 162/63 readiness records only after evidence supports it.


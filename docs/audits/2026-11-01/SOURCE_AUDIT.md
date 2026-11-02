# Source and Evidence Audit — 1 November 2026

## Scope and provenance

This report covers the foundation crates and merged hosting, database, backup, DNS, node, security, and observability component changes available during the 1 November 2026 review. The principal source baseline was master 8dc6bdd19e2fa2e32d87fbad7c50898945d50a24. The audit documentation branch starts at 3a8bf9ef39c35a43b36771c575dbc3b11bc601ba; the intervening PR #89 changes documentation only.

Findings below are static source findings unless a separate executed-test result is explicitly recorded. The audit did not apply host firewall/SSH configuration, restore production data, contact production services, or prove production exploitation. A passing package suite does not establish integrated feature acceptance.

Related parent issues: [S06 foundation #6](https://github.com/scherenhaenden/FusionPanel/issues/6), [S08 identity #11](https://github.com/scherenhaenden/FusionPanel/issues/11), [S09 hosting #12](https://github.com/scherenhaenden/FusionPanel/issues/12), [S10 databases #13](https://github.com/scherenhaenden/FusionPanel/issues/13), [S11 backups #15](https://github.com/scherenhaenden/FusionPanel/issues/15), [S13 DNS #16](https://github.com/scherenhaenden/FusionPanel/issues/16), [S15 security #23](https://github.com/scherenhaenden/FusionPanel/issues/23), and [S16 observability #22](https://github.com/scherenhaenden/FusionPanel/issues/22).

## Executed foundation checks

| Check | Observed result | Limit |
|---|---|---|
| cargo fmt --all -- --check | PASS, exit 0 | Foundation workspace formatting only. |
| cargo test --workspace --offline --locked in the repository | BLOCKED, exit 101: root Cargo.lock requires an update | Cargo.lock is tracked. Its presence in .gitignore does not untrack it. The audit did not regenerate or edit the repository lockfile. |
| Copied workspace, offline dependency resolution, tests inside the sandbox | Source compiled; protocol 14/14 and API 5/5 passed; CLI had 3 passes and 8 fixture failures caused by denied loopback binding | Environment failure, not demonstrated CLI regressions. |
| Same copied-workspace test outside the sandbox, with offline resolution | PASS, exit 0: 49 tests, 0 failed | Resolution updated only the disposable copy's lockfile. No actual node worker, full product API, or production service was exercised. |

The 49 passing tests comprise 14 protocol, 5 API, 11 CLI, 10 domain, and 9 job-store tests. The copy-only resolution added 182 packages relative to the stale root lock. Both temporary workspace copies and their build artifacts were removed. No repository source or lockfile was changed by these checks.

Module test counts quoted elsewhere in project records were not rerun as part of this source audit. They remain previously reported evidence tied to their reviewed source revisions.

## Severity and acceptance gates

- P1 identifies a component defect that blocks activation of the affected runtime path and release acceptance until repaired and meaningfully verified.
- P2 identifies a correctness or evidence gap that must be resolved before accepting the associated capability.
- An inert model or configuration generator can contain a P1 activation defect even though the integrated product path does not yet exist.
- BLOCKED, ignored, and not-run cases receive no real integration or E2E acceptance credit.

## Foundation integration — issue #6

| ID | Priority | Source evidence | Required behavior |
|---|---|---|---|
| FND-01 | P1 release gate | The root workspace declares five members but the tracked lock cannot satisfy a locked offline test. There is no foundation CI workflow at the audited baseline. | Review and commit the reconciled root lock, establish CI, and run locked workspace tests, formatting, and clippy. Preserve honest dependency failures. |
| FND-02 | P2 | [Canonical domain validation](../../../crates/domain/src/domain_name.rs#L34) rejects alternate IPv4 forms and normalizes case; [protocol validation](../../../crates/agent-protocol/src/lib.rs#L116) uses a separate label validator and plain IpAddr parsing. Numeric forms such as 127.1 pass the latter boundary. | Add explicit checked conversion or a shared canonical validator. Test accepted names, canonicalization, numeric aliases, and invalid paths at both boundaries. |
| FND-03 | P2 | [JobId is a String alias](../../../crates/jobs/src/models.rs#L12), while [domain JobId is a UUID newtype](../../../crates/domain/src/id.rs#L72). [Stored rows](../../../crates/jobs/src/store.rs#L190) are read without UUID validation. | Define checked adapters and typed malformed-ID errors. Preserve the established JSON string representation. Do not conflate request IDs and job IDs solely because both use UUIDs. |
| FND-04 | P2 | [API routes](../../../crates/api/src/main.rs#L112) expose health and capability discovery only. Foundation manifests have no cross-crate integration adapters. | Wire the bounded foundation contracts and add integration fixtures; do not advertise absent hosting/worker operations. |
| FND-05 | P1 release gate | [Running-job inspection](../../../crates/jobs/src/store.rs#L84) exists and correctly does not automatically retry. Existing tests exercise reopen and concurrent claims, but no worker kill/restart reconciliation journey exists. | Add a disposable process-level persistence/reconciliation case. Running work must survive interruption and remain explicit; do not fabricate retry success or duplicate an external side effect. |
| FND-06 | P2 | [Request validation](../../../crates/agent-protocol/src/lib.rs#L82) is explicit; [responses](../../../crates/agent-protocol/src/lib.rs#L40) carry version/request ID without an integrated consumer checking them. | Validate response version, request correlation, and operation result shape before future job transitions. |

## Hosting — issue #12

### HST-01 — P1: generated configuration accepts untrusted directives

[Document-root validation](../../../modules/hosting/src/domain/validation.rs#L57) checks prefix, NUL, traversal substring and absoluteness, but permits whitespace, line breaks and configuration delimiters. [Site and strategy models](../../../modules/hosting/src/domain/site.rs#L12) expose unchecked PHP/proxy fields and public mutable state; derived deserialization bypasses constructors. [Nginx rendering](../../../modules/hosting/src/config/nginx.rs#L16) and [PHP-FPM rendering](../../../modules/hosting/src/config/php_fpm.rs#L18) interpolate those values directly.

A caller can therefore obtain a supposedly valid model whose document root, upstream URL, pool name or memory setting introduces additional configuration syntax. Enforce validation at construction, deserialization and final rendering, with immutable invariant-bearing fields and deliberate escaping or rejection rules.

### HST-02 — P1 activation gate: generated PHP pools share the same identity

[PHP-FPM generation](../../../modules/hosting/src/config/php_fpm.rs#L13) computes a tenant identity but emits www-data for every pool. This does not implement per-user isolation. Require an explicit validated ownership mapping, or report the operation unsupported until one exists; do not invent host accounts.

### HST-03 — P2: BLOCKED cases return a passing test

The [syntax test](../../../modules/hosting/tests/integration.rs#L61) returns normally when Docker cannot run, and the [E2E case](../../../modules/hosting/tests/e2e.rs#L7) prints BLOCKED then asserts true. Green aggregate counts cannot qualify parser checks or provisioning. Missing prerequisites must produce an explicit nonzero qualification result. Real acceptance must serve uploaded content and verify compensation against disposable services.

Observed coverage is hosting specifications and Nginx/PHP-FPM configuration strings for static, PHP, and reverse-proxy sites. Provisioning, serving, compensation and isolated tenant identities remain unqualified.

## Database adapters — issue #13

| ID | Priority | Source evidence and consequence | Repair requirement |
|---|---|---|---|
| DB-01 | P1 | [Public deserializable plans](../../../modules/databases/src/adapter.rs#L78) can bypass plan-method validation. MariaDB apply methods at [create](../../../modules/databases/src/mariadb.rs#L75), [user](../../../modules/databases/src/mariadb.rs#L97), and [grant](../../../modules/databases/src/mariadb.rs#L133), and equivalent PostgreSQL methods, interpolate unchecked fields into SQL. | Make plans checked/immutable and revalidate at apply. Use engine-specific safe SQL handling. Include direct construction and deserialization negative cases. |
| DB-02 | P1 | [PostgreSQL user creation](../../../modules/databases/src/postgresql.rs#L94) and [restore](../../../modules/databases/src/postgresql.rs#L199) feed stdin into psql without ON_ERROR_STOP and rely on process status. A SQL error can leave script processing running and yield apparent success. | Set explicit error-stop behavior, verify SQL-level outcomes, and test a deliberately failing restore without claiming success or silently changing passwords. |
| DB-03 | P2 | [Shared privilege validation](../../../modules/databases/src/adapter.rs#L150) accepts table privileges such as SELECT, but [PostgreSQL apply_grant](../../../modules/databases/src/postgresql.rs#L151) applies them ON DATABASE. | Model engine-specific database/schema/table privileges and test actual authorization as the created application user. |
| DB-04 | P2 | [PostgreSQL host command setup](../../../modules/databases/src/postgresql.rs#L15) ignores socket_path; container execution disregards the configured connection endpoint. Dump operations create/truncate output before successful completion. | Honor or explicitly reject unsupported connection strategies. Protect dump paths, permissions and existing files; clean partial outputs and sanitize errors. |
| DB-05 | P2 | The [ignored integration wrapper](../../../modules/databases/tests/integration_test.rs#L159) swallows DependencyUnavailable; the [container helper](../../../modules/databases/tests/integration_test.rs#L153) discards the connection result. Restore checks use another database on the same instance and insert no application records. | Fail qualification on missing prerequisites. Use dedicated disposable source and restore instances, real records and grants, hash/row assertions, and complete teardown. |

The [psql documentation](https://www.postgresql.org/docs/current/app-psql.html#APP-PSQL-VARIABLES) confirms that processing continues after an error by default and documents the nonzero behavior enabled by ON_ERROR_STOP. This report does not claim a live database test was run.

## Backup and recovery core — issue #15

| ID | Priority | Source evidence and consequence | Repair requirement |
|---|---|---|---|
| BAK-01 | P1 | [Restore lexical checks](../../../modules/backups/src/restore.rs#L47) are followed by create_dir_all and File::create. Existing target symlinks can redirect writes outside the selected target. Source traversal at [backup.rs](../../../modules/backups/src/backup.rs#L91) follows symlinks too. | Enforce anchored source/destination containment and explicit symlink policy using safe APIs. Test parent and leaf symlinks in temporary fixtures. |
| BAK-02 | P1 | [Manifest loading](../../../modules/backups/src/restore.rs#L33) builds an expected-file map, but [completion](../../../modules/backups/src/restore.rs#L93) never verifies every manifest member appeared. Sizes, duplicate manifests and duplicate file entries are not validated. | Reject incomplete, duplicate and inconsistent archives; verify complete inventory and actual sizes before reporting success. |
| BAK-03 | P1 | [Restore creates/truncates](../../../modules/backups/src/restore.rs#L65) before checking the checksum and [removes the target](../../../modules/backups/src/restore.rs#L81) on mismatch. | Stage validated output and commit safely. A failed restore must preserve existing valid target data. |
| BAK-04 | P2 | [Backup traversal](../../../modules/backups/src/backup.rs#L83) treats a missing/non-directory source as empty; output created inside the source tree can enter the second traversal without a manifest entry. | Reject invalid sources and source/output overlap; prevent successful un-restorable archives and two-pass inconsistency. |
| BAK-05 | P2 | [Retention](../../../modules/backups/src/retention.rs#L15) identifies candidates by .gz extension and deletes them without establishing backup ownership. | Restrict retention to validated owned backup artifacts and preserve unrelated/corrupted inputs unless an explicit reviewed cleanup policy applies. |

Source supports file archives, checksum comparison, whole-archive restore and count/age pruning. Database information is metadata, not an executed dump. README labels do not establish encryption, selectable restore, scheduling, Restic, pre-update orchestration, domain rewrite or a dashboard. Existing filesystem cases cover a happy file restore, count pruning and one checksum mismatch; they omit the failure paths above.

## DNS regression preservation — issue #16

The merged PR #85 implementation was compared directly with reviewed but unmerged source f6df6f202323ec32a46cad5db353a4cd2e5a5ca0 from PR #84. These are source-confirmed regressions; the word superseded does not establish preservation of the earlier behavior.

| ID | Priority | Current source | Regression and gate |
|---|---|---|---|
| DNS-01 | P2 | [Record::new](../../../modules/dns/src/domain.rs#L164) rejects TTL zero. | Earlier source allowed TTL zero. Restore valid zero-TTL handling and boundary tests. |
| DNS-02 | P2 | [MX parsing](../../../modules/dns/src/domain.rs#L183) accepts any u16 priority with the root target; [zone insertion](../../../modules/dns/src/domain.rs#L256) has no Null MX exclusivity rule. | Earlier source required priority zero for Null MX and rejected coexistence with another MX at the same owner. Preserve both rules. |
| DNS-03 | P1 integration gate | [Record fields](../../../modules/dns/src/domain.rs#L140) and [zone identity/name/tenant fields](../../../modules/dns/src/domain.rs#L210) are public. | Validated objects can be mutated into invalid state; changing the zone name invalidates existing ownership relationships. Restore private invariant-bearing state and checked mutation APIs. |
| DNS-04 | P2 | [DnssecKey](../../../modules/dns/src/provider.rs#L6) contains a generic digest string. | Earlier source distinguished typed DNSKEY public-key material from DS digest material and explicit encodings. Preserve that contract instead of collapsing incompatible records. |
| DNS-05 | P2 | [Name validation](../../../modules/dns/src/domain.rs#L89) checks only ordinary IpAddr parsing before removing the root dot. | Alternate numeric IPv4 forms and a dotted address with a trailing root dot can bypass the check; earlier source canonicalized first and rejected legacy aliases. |

No authoritative provider/backend or real DNS create/update/query/export/restore qualification is claimed. The six tests reported for PR #85 must not be described as the eleven tests from the earlier PR #84 source.

## Security policies — issue #23

### SEC-01 — P1 activation gate: CSP checks do not parse the emitted token stream

[CSP validation](../../../modules/security/src/csp.rs#L44) recognizes unsafe-inline only when an entire value equals that token, while [rendering](../../../modules/security/src/csp.rs#L93) joins values with spaces. A value containing multiple tokens can bypass the exact-value check. The nonce/hash test at lines 56–61 recognizes only a prefix, so malformed nonce/hash text can satisfy the mitigation check. Parse and validate source expressions and their encodings as tokens; reject malformed combined strings and fake nonce/hash prefixes.

### SEC-02 — P1 activation gate: lockout prevention ignores effective rule matching

[Firewall validation](../../../modules/security/src/firewall.rs#L165) sets a boolean when it sees any allow-TCP rule covering the SSH port. It does not evaluate deny rules, ordering or whether the emergency operator's source is actually covered. A preceding deny, an allow for an unrelated source, or an explicit SSH deny under a default-allow policy can still validate. Define the adapter's effective rule semantics, preserve the designated recovery source, and test the actual disposable firewall path before activation.

### SEC-03 — P2: three diagnostic tests count as passing integration cases

[Three non-ignored cases](../../../modules/security/tests/integration_stubs.rs#L57) print BLOCKED for parser validation, benign/malicious traffic and emergency SSH, then return normally. These are useful diagnostics but are not passed integration/E2E cases. The qualification command must emit machine-readable status and fail when any required case is blocked.

No host firewall/SSH/WAF change was applied by this audit. Existing policy-preview/unit evidence remains distinct from parser, traffic and recovery evidence.

## Observability — issue #22

- OBS-01, P2: [TCP resolution](../../../modules/observability/src/probes.rs#L43) selects only the first resolved address. A hostname can be reported down when its first address is unreachable but another advertised address is reachable. Try eligible addresses under one bounded deadline and record the attempt outcome.
- OBS-02, P2 evidence gap: a real loopback receiver exists in [test_webhook_delivery_success](../../../modules/observability/tests/integration.rs#L109). The receiver reads bytes but does not assert HTTP method, headers or the serialized alert payload. Strengthen those assertions and failure responses; do not describe this as a missing receiver.
- No unlimited-webhook-timeout finding is asserted. The coordinator checked reqwest's default client timeout of 30 seconds. Explicit product timeout requirements may still be designed later, but absence of a custom setting is not proof of an unbounded request.
- Collection, scheduling, durable incidents and the full outage-to-alert-to-recovery journey remain separate runtime/E2E acceptance gates.

## Recovered identity artifact — issue #11

This item is coordinator-reported static evidence from an unmerged recovery artifact, not independently inspected master source.

- Recovered Jules session: 3198712460135711531.
- Draft PR: [#90](https://github.com/scherenhaenden/FusionPanel/pull/90), head ff4caef9a7eba8f9f60b8e570e182a92fdefd325.
- The recovery originally contained changes across 117 paths; only modules/identity was scoped into the recovery PR.
- Compilation with zero tests is not identity/RBAC/session acceptance.
- The coordinator reports public invariant-bearing fields and unchecked session-TTL arithmetic that can panic for extreme lifetimes. These require direct source confirmation and regression coverage before integration; no confirmed production exploit is asserted.

Gate the draft on checked state construction/deserialization, bounded expiry calculations, real persistence reopen, tenant/site permission negatives, revocation/demotion and secret-safe evidence. Qualify the recovered branch rather than launching a duplicate session solely to request a PR.

## Recovered secrets artifact — issue #25

Draft [PR #98](https://github.com/scherenhaenden/FusionPanel/pull/98), head 18c65b8e5a1e84c03a34791540d2dbd2acf9e034, recovers session 1788723557430535400. The original artifact had 783 paths; 775 generated build-artifact paths and the module lockfile were excluded. Eight source/configuration/documentation files were retained.

The recovery reviewer executed eight crypto/filesystem tests successfully and excluded one false-green E2E placeholder. The draft remains blocked on raw-key Debug disclosure, unchecked identifier path traversal, tenant/scope key collisions, unauthenticated metadata and state access, non-atomic rotation/write recovery, restricted file permissions, and proof that a rotated store reopens with the new key. These are unmerged recovery-source findings documented in the PR/README; no production exposure or live exploit is asserted.

Qualify this recovered source additively in modules/secrets/**. Require actual crypto/persistence and isolation regressions, failure injection preserving recoverability, new-key reopen and old-key rejection, log redaction, and a genuine service-injection/recovery E2E or nonzero BLOCKED result.

## Bounded repair ownership

The coordinator owns issue creation, dispatch prompts, exact base SHAs, session IDs, PR delivery and the 15-active-session ceiling. This table defines independent source ownership only.

| Repair | Parent issue | Exclusive implementation paths | Required exit evidence |
|---|---|---|---|
| Foundation integration and reproducible CI | #6 | Root workspace/lock files, crates/**, a dedicated foundation CI workflow and tests/foundation/** | Locked workspace tests/format/clippy; checked identifier/domain adapters; actual process persistence/reconciliation; no fake runtime success. |
| Hosting boundary and truthful qualification | #12 | modules/hosting/** | Injection/deserialization/identity negatives, real parser checks or nonzero BLOCKED, and explicit unimplemented runtime boundaries. |
| Database execution and independent restore | #13 | modules/databases/** | Checked apply plans, PostgreSQL error-stop and privilege semantics, protected dumps, real separate source/restore instances and application record/grant assertions. |
| Backup containment and restore integrity | #15 | modules/backups/** | Symlink/completeness/duplicate/corruption regressions, preserved old target on failure, safe retention and real filesystem evidence. |
| DNS regression restoration | #16 | modules/dns/** | Preserve earlier TTL/Null MX/typed DNSSEC/alias and mutation invariants; real provider evidence or nonzero BLOCKED. |
| Security policy semantics and qualification status | #23 | modules/security/** | Parsed CSP expressions, effective firewall recovery access, and required blocked diagnostics failing qualification. |
| Observability probe and webhook evidence | #22 | modules/observability/** | Multi-address TCP deadline cases, captured alert payload assertions and actual outage/recovery evidence when runtime prerequisites exist. |
| Secrets recovery qualification | #25 | modules/secrets/** on the reviewed recovery branch | Key redaction, checked identifiers/authorization, collision-safe authenticated metadata, atomic rotation and crash/reopen evidence. |
| Identity recovery qualification | #11 | modules/identity/** on the reviewed recovery branch | Nonzero meaningful tests for invariants, extreme TTL, RBAC isolation, persistence and revocation; independently inspect coordinator-reported findings. |

Each repair must preserve existing feature IDs and selectable user workflows, state unsupported behavior explicitly, add meaningful negative cases, report dependency versions and cleanup, and create a focused PR against master using Refs with the correct parent issue. Parallel sessions must not share write paths. No repair session merges its own PR or qualifies a blocked dependency as a pass.

## Execution handoff

Audit repairs are tracked by issues #91–#97; the exact prompts, base commits, recovered branch heads, and session IDs are maintained in [wave W02](../../execution/waves/W02_2026-11-01/README.md). The source audit does not itself mark any capability accepted.

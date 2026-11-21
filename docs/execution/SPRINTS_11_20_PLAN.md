# FusionPanel delivery plan for Sprints 11–20

Planning checkpoint · 22 November 2026 · Europe/Berlin

Sprint 11 follows the scope already recorded in the [sprint roadmap](SPRINTS.md). Sprints 12–20 propose an order for the existing [work packages](DELIVERY_PLAN.md) and [capability inventory](READINESS_CATALOG_2026-11-17.md). Each sprint delivers a workflow that an operator can demonstrate on a disposable environment. The estimates are relative and include fixture work. Confirm capacity, owner and lab availability at kickoff before assigning dates.

## Handoff from Sprint 10

- GitHub `master` is `a8861e6110efaae7dbc589474335ca98c93a44c2`, the Sprint 9 merge. GitHub `sprint-10` is `7516b972cfc27bd66f0d4c2a241aa5058375e2ff`. These are planning references. Refresh both before kickoff.
- The candidate has focused product-process evidence for telemetry and loopback HTTP outage/recovery. The P1 audit corrections for global observability authorization, monitor cancellation and durable alert delivery have focused local tests. They still need review, integration and qualification on the sprint candidate. The last full gate in the sprint record was run on an earlier candidate. [Sprint 10 evidence](SPRINT_10_PLAN.md)
- Close the release gate with an actual agent and Dashboard browser run, an API interruption while a signed webhook is pending, recovery with the same delivery ID, and cancellation of a down monitor without a recovery notification. Verify a read-only or tenant-scoped operator cannot access global observability. Recheck non-observability workflows on the same candidate. [#51](https://github.com/scherenhaenden/FusionPanel/issues/51) · [#683](https://github.com/scherenhaenden/FusionPanel/issues/683) · [#684](https://github.com/scherenhaenden/FusionPanel/issues/684) · [#685](https://github.com/scherenhaenden/FusionPanel/issues/685)
- Record a disposition for the remaining audit findings before release. Cover encryption-key provisioning, the effect of slow deliveries on job/monitor progress, stale responses when switching nodes in the Dashboard, and rerunning documented locked commands from a clean checkout. Recheck installer behavior when the key is missing or persisted across an update.
- Default sequence is Sprint 10 acceptance and merge, then Sprint 11 from verified `master`. Sprint 11 fixture and contract design can proceed in preparation. Any change in release order requires confirming the base and version assignment.

## Roadmap and dependencies

The dependency column lists the product prerequisites that drive the order. Every implementation sprint branches from the then-verified `master` under the existing sprint workflow.

| Sprint | Operator outcome | Depends on | Required real fixture | Estimate |
| --- | --- | --- | --- | --- |
| [11](#sprint-11--mail-core-and-debian-package) | Create and use a mailbox and install a reproducible Debian package | 4, 7 and Sprint 10 handoff | Postfix/Dovecot, authoritative DNS, Debian VM | 13 |
| [12](#sprint-12--safe-upgrade-and-rollback) | Upgrade a running install and recover from a bad candidate | 11 | Debian VM, two verified artifacts | 13 |
| [13](#sprint-13--single-node-containers) | Run and manage a container from the panel | 2, 9, 12 | Disposable Docker daemon, registry, browser | 13 |
| [14](#sprint-14--container-deployments) | Promote and roll back an immutable application revision | 3, 4, 13 | OCI fixture, Docker, Nginx, browser | 13 |
| [15](#sprint-15--wordpress-lifecycle) | Provision and restore a managed WordPress site | 3–6, 12, 13 | Pinned WordPress runtime, MariaDB, MinIO, browser | 13 |
| [16](#sprint-16--scheduled-backup-and-independent-restore) | Schedule and verify recoverable PostgreSQL and MariaDB backups | 5, 6, 12 | Both database engines, MinIO, browser | 13 |
| [17](#sprint-17--remote-nodes-and-failure-recovery) | Enroll two remote nodes and recover jobs safely when one disconnects | 2, 9, 10, 12 | Two Debian node VMs plus control plane | 13 |
| [18](#sprint-18--host-security-and-secret-rotation) | Apply and roll back host access policy and rotate a service secret | 9, 12, 17 | Debian VM, out-of-band console, two test clients | 13 |
| [19](#sprint-19--assisted-migration-and-cutover) | Move a representative site and database, then rehearse rollback | 12, 16–18 | Two full OS guests, DNS fixture, legacy export | 16 |
| [20](#sprint-20--bounded-release-candidate) | Reproduce, operate and recover a release candidate from the handbook | 11–19 | Clean Debian, upgrade/recovery fixtures, CI runners | 13 |

Start booking the Debian and two-node lab during Sprint 11. Pull fixture preparation forward when the contract is stable and the write paths are independent. Keep a single owner for edits to shared CLI/API contracts, protocol, workspace manifests and lockfiles. Land the contract before branches that consume it. Reserve capacity for failed qualification and rework. If scope exceeds capacity, split at a demonstrable user workflow and record the deferred coverage.

## Sprint workflow and evidence

| Stage | Required result |
| --- | --- |
| Kickoff | Verify the base, create `sprint-N` from `master`, assign an integration owner and fixture owner, confirm the issue scopes and the environment, and record the applicable feature IDs and supported modes. |
| Atomic PR | Branch from `sprint-N`. Own the listed paths, add focused cases for the changed behavior, and run the affected crate, module or frontend unit tests. Keep standalone packages and tracked locks consistent with repository policy. |
| Integration | Merge reviewed atomic PRs into `sprint-N`. For each UI/API/CLI journey inspect real service effects, tenant denial, retry behavior and persistence where applicable. Failures return to atomic fix PRs. |
| Qualification | Freeze the versioned head and base SHA. Run affected integration suites, product E2E, workspace regression and quality checks required by the sprint. Exercise each supported mode separately and record any excluded variant. |
| Release handoff | Record results and outstanding gaps in the roadmap, tracker and capability evidence. A fully qualified candidate gets one regular release PR to `master` following branch policy. Verify the reviewed head, required checks, packaging and merge SHA before recording closure. |

The build and test commands below are entry points or proposed fixtures, identified as such where they do not exist. At kickoff, record actual commands for that branch. Use the [test strategy](../testing/TEST_STRATEGY.md) and [AGENTS.md](../../AGENTS.md) for runner preflight, Docker storage accounting, offline local checks and teardown. Use a disposable full OS fixture for systemd, reboot, firewall and machine recovery. Preserve an evidence bundle with head/base SHA, versions, commands, run times, test results, sanitized logs, fixture cleanup and limitations. Record `PASS`, `FAIL`, `BLOCKED` or `NOT_RUN` for each applicable gate.

## Sprint 11 · Mail Core and Debian package

**Demo.** Create an owned mail domain, mailbox and alias in the panel, publish DKIM TXT and verify a real signed SMTP/IMAP roundtrip. Install the same candidate from a reproducible Debian artifact, sign in and observe the agent reconnect after reboot.

The [Sprint 11 implementation plan](SPRINT_11_PLAN.md) contains the Mail API/job, UI, agent/signing and Debian package work packages, write paths, commands, two E2E lanes and ticket mappings. Start the mail contract and packaging independently. Book the Debian guest and reusable SMTP/IMAP/DNS fixture before implementation. The release gate requires the supported one-node mail journey and Debian installation, plus independent build hashes. [#686](https://github.com/scherenhaenden/FusionPanel/issues/686) · [#687](https://github.com/scherenhaenden/FusionPanel/issues/687) · [#688](https://github.com/scherenhaenden/FusionPanel/issues/688)

## Sprint 12 · Safe upgrade and rollback

**Demo.** Upgrade a working Debian install, see its tenant, node and sites return, then restore the previous working release after a deliberately rejected candidate. Reuse [packaging](../../packaging/) and the [updater](../../packaging/updater/). [S38](sessions/S38.md) · [S51](sessions/S51.md) · [#42](https://github.com/scherenhaenden/FusionPanel/issues/42) · [#53](https://github.com/scherenhaenden/FusionPanel/issues/53)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-12-upgrade-preflight` · 3 | Validate candidate identity, supported source version, platform, state paths and pre-update disk/config; use the release metadata already defined by the updater. `packaging/updater/` | Wrong version, corrupt artifact, duplicate request, insufficient space and unchanged install after failed preflight. |
| `feat/sprint-12-upgrade-rollback` · 5 | Stage install, preserve config and durable state, activate the candidate, check API and agent health, and retain a recoverable previous version. Record the compatibility rule for each state migration. `packaging/` outside updater internals after the preflight contract | Simulated activation failure, repeat rollback, credentials and state preserved. Use `bash packaging/tests/test_update_layout.sh` and affected updater tests. |
| `test/sprint-12-update-vm` · 5 | Build a disposable two-release Debian VM lane, with a failed candidate, interruption case and recorded teardown. `tests/e2e/administration/` | Fixture wiring and evidence schema before sprint-close VM tests. |

**Release gate.** In the installed product, seed a tenant and live served site. Upgrade and assert stable IDs, site content, login and agent health. Reject a bad-health candidate and prove the previous release serves the same data after reboot. Interrupt at least one staging or activation boundary and verify operator recovery. Save manifests, artifact hashes, state comparisons and elapsed time. Use supported adjacent versions and a disposable install. Major jumps, arbitrary downgrades and alternate distributions stay outside this lane. An incompatible schema migration blocks automatic rewind until a restore path is demonstrated.

## Sprint 13 · Single-node containers

**Demo.** An operator starts an approved pinned image, reads state/logs, stops and restarts the workload, then removes it without affecting another tenant. Reuse [container integration](../../integration/containers/) and [the app catalog module](../../modules/app-catalog/). [S28](sessions/S28.md) · [S40](sessions/S40.md) · [S45](sessions/S45.md) · [#30](https://github.com/scherenhaenden/FusionPanel/issues/30) · [#48](https://github.com/scherenhaenden/FusionPanel/issues/48)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-13-container-contract` · 3 | Tenant-owned workload, image reference, bounded resource settings, state, request/result and job lifecycle. `crates/api/`, `crates/control-store/`, `crates/agent-protocol/` | Ownership, normalized ports, idempotent requests and honest failure states. |
| `feat/sprint-13-container-worker` · 5 | Node-scoped Docker adapter for managed start/stop/remove, collision handling, restart reconciliation and bounded logs. `crates/agent/`, `integration/containers/`, `modules/containers/` | Apply/error mapping, container ownership labels, retry without duplicate containers and no unmanaged deletion. |
| `feat/sprint-13-container-ui` · 2 | Catalog selection, status, logs and lifecycle actions through product API. `frontend/` | Read-only role, failures, stale status, confirmed delete and log limits. |
| `test/sprint-13-container-e2e` · 3 | Disposable Docker registry and product browser fixture. Proposed `tests/e2e/containers/` lane | Pin fixture inputs and verify cleanup only touches test-owned, labeled objects. |

**Release gate.** Launch the digest-pinned image, read content from the actual container, verify stop/restart/remove and port release, reject a collision and foreign-tenant access, and restart API/agent while preserving workload identity. Use a separate unmanaged sentinel workload to verify no destructive cross-effect. Sprint scope is single-node and a bounded set of supported options. Arbitrary Compose files, privileged workloads, host mounts and catalog image scanning stay outside the release claim.

## Sprint 14 · Container deployments

**Demo.** Promote a revision behind a managed HTTPS endpoint, reject a failed revision, then roll back to a previously healthy digest. Reuse [deployment integration](../../integration/deployments/) and Sprint 13 container jobs. [S30](sessions/S30.md) · [S46](sessions/S46.md) · [#33](https://github.com/scherenhaenden/FusionPanel/issues/33) · [#49](https://github.com/scherenhaenden/FusionPanel/issues/49)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-14-release-contract` · 3 | Tenant-scoped project/revision state and promotion request for one trusted OCI digest, with source metadata, history and explicit rollback. `crates/api/`, `crates/control-store/`, `crates/agent-protocol/` | Replay/conflicts, foreign project, invalid or mutable digest and out-of-order job result. |
| `feat/sprint-14-deploy-worker` · 5 | Stage candidate alongside active revision, check readiness, switch routing and retain prior revision. Recover interrupted promotion and reconcile uncertain results. `crates/agent/`, `integration/deployments/`, `modules/deployments/` | Failed health, failed switch, duplicate jobs and prior route retention. |
| `feat/sprint-14-deploy-ui` · 2 | Revision history and promote/rollback actions with state and failure messages. `frontend/` | Confirmation, authorization, queued versus active and error recovery. |
| `test/sprint-14-deployment-e2e` · 3 | Build two healthy fixtures and one failing digest, then exercise them through the real route. `tests/e2e/deployments/` | Fixture repeatability and evidence for revision identity and traffic. |

**Release gate.** Serve revision A, promote B, observe B through TLS, attempt the unhealthy revision and verify B remains live, then roll back to A. Replay and restart once mid-flow, verify one active revision, and check another tenant cannot promote or fetch private history. The first workflow consumes a prebuilt image. Arbitrary build scripts, external Git provider webhooks and orchestrated multi-node rollouts stay deferred.

## Sprint 15 · WordPress lifecycle

**Demo.** Create a managed WordPress site, reach real wp-admin, run a verified backup and recover site content after an update or restore scenario. Use the supported container app integration so the standard workflow has explicit runtime and version boundaries. Reuse [WordPress integration](../../integration/wordpress/) and existing hosting, MariaDB and backup paths. [S22](sessions/S22.md) · [S33](sessions/S33.md) · [S47](sessions/S47.md) · [#27](https://github.com/scherenhaenden/FusionPanel/issues/27) · [#36](https://github.com/scherenhaenden/FusionPanel/issues/36) · [#50](https://github.com/scherenhaenden/FusionPanel/issues/50)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-15-wordpress-api` · 3 | Tenant-scoped install and lifecycle contract, idempotent reservation of site and database, status and redacted outputs. `crates/api/`, `crates/control-store/`, `crates/agent-protocol/` | Foreign site/database, credential redaction, retries, duplicate domain and partial failure. |
| `feat/sprint-15-wordpress-worker` · 5 | Provision supported pinned runtime with managed TLS/database, map file and DB dependencies to approved backup flows, and provide bounded updates with an explicit restore point. `crates/agent/`, `integration/wordpress/`, `modules/wordpress/` | Staged config, health/failure handling, rollback and unchanged unrelated sites. |
| `feat/sprint-15-wordpress-ui` · 2 | Guided setup, health, backup/restore status and explicit update compatibility. `frontend/` | Blank/loading/failure states, permission checks and no secret persistence in the UI. |
| `test/sprint-15-wordpress-e2e` · 3 | Pinned real WordPress, MariaDB, TLS and MinIO fixture. `tests/e2e/wordpress/` | Synthetic site data, contract checks and deterministic cleanup. |

**Release gate.** Create a site from the browser, sign in to WordPress, write a post and upload an asset, then back up and restore to an isolated target. Assert post rows, media hashes, served content and tenant isolation. Test a failed update with retained working data and record the exact supported upgrade pair. General PHP management, multisite, arbitrary theme/plugin imports and managed updates outside the tested version pair stay out of scope.

## Sprint 16 · Scheduled backup and independent restore

**Demo.** Configure a backup schedule and restore a selected generation to an independent database, proving that the original remains untouched and the restored data is readable. Reuse [data integration](../../integration/data/) and the existing S3-compatible backup path. [S27](sessions/S27.md) · [S39](sessions/S39.md) · [S44](sessions/S44.md) · [S56](sessions/S56.md)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-16-backup-schedule` · 3 | Durable schedule/timezone and bounded retention policy, overlapping run behavior, cancellation and status. `crates/api/`, `crates/control-store/`, `integration/data/` | Restart, skipped or duplicate slot, retention hold and tenant scope. |
| `feat/sprint-16-independent-restore` · 5 | Validate manifest/digest, reserve a separate target, restore files/database with least privilege, and clean failed stages. `crates/agent/`, `modules/backups/`, `modules/databases/` | Wrong engine/target, corrupted artifact, retries, cancellation and source preservation. |
| `feat/sprint-16-backup-ui` · 2 | Schedule, last/next run, retained generations and an explicit independent restore target. `frontend/` | Scheduling validation, failure history, restore confirmation and access. |
| `test/sprint-16-data-recovery-e2e` · 3 | Real PostgreSQL, MariaDB and MinIO with scheduled run, corruption and restoration cases. `tests/e2e/data/` | Fixtures pin both engine versions and seed rows and files with known checksums. |

**Release gate.** Exercise both database engines separately. Observe a real scheduled run, restore to fresh targets and verify rows, integrity, grants and source state. Restart mid-schedule and confirm one job per slot. Corrupt a fixture artifact and verify rejection before promotion. Keep the last usable generation when retention or destination cleanup fails. Record elapsed restoration and recovery-point age per engine and dataset. Native provider qualification, FTPS/SFTP and multi-region recovery remain separate work.

## Sprint 17 · Remote nodes and failure recovery

**Demo.** Enroll two remote agents, direct a workload to an eligible node, observe a node disconnect and recover the job without duplicate execution. Reuse the existing product HTTP agent and adopt compatible parts of [node integration](../../integration/nodes/). [S21](sessions/S21.md) · [S34](sessions/S34.md) · [S49](sessions/S49.md) · [#37](https://github.com/scherenhaenden/FusionPanel/issues/37) · [#52](https://github.com/scherenhaenden/FusionPanel/issues/52)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-17-remote-enrollment` · 3 | Durable node identity, explicit management scope, transport authentication and credential revocation. Define compatibility for existing agents and job authorization. `crates/api/`, `crates/control-store/`, `crates/agent-protocol/` | Enrollment replay, wrong node, credential revocation and API restart. |
| `feat/sprint-17-job-reconciliation` · 5 | Atomically fence claims, keep node and job progress observable and reconcile uncertain external effects before retry or reassignment. `crates/jobs/`, `crates/agent/`, `modules/nodes/`, `integration/nodes/` | Duplicate claims, expired/obsolete completion, unreachable node and safe same-node retry. |
| `feat/sprint-17-nodes-ui` · 2 | Show health, last heartbeat, eligibility, current job owner and explicit recovery actions. `frontend/` | Stale/offline distinction, forbidden action and consistent progress after reload. |
| `test/sprint-17-node-failure-e2e` · 3 | Two VM agents with temporary identities, controlled disconnect and state restoration. `tests/e2e/nodes/` | Bounded fault controls and isolated job/credential fixtures. |

**Release gate.** On two real node VMs, enroll, restart and reconnect both agents. Interrupt a job during a node partition, observe the timeout, reconcile the first attempt and allow a safe retry only when the first effect is known. Verify the affected tenant cannot act on the other tenant’s resources and that a revoked node is refused on reconnect. Report recovery and detection intervals and preserve a before/after inventory. Automatic workload movement, shared storage failover, HA and autoscaling remain separate qualifications.

## Sprint 18 · Host security and secret rotation

**Demo.** Apply a restricted policy to one managed node, verify its allowed service and operator access, undo a failed change, and rotate one managed service credential with a recorded audit trail. Reuse [security integration](../../integration/security/) and [secrets](../../modules/secrets/). [S15](sessions/S15.md) · [S23](sessions/S23.md) · [S37](sessions/S37.md) · [S52](sessions/S52.md)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-18-policy-and-audit` · 3 | Permissioned, revisioned preview/apply/rollback API for one supported firewall policy and one managed secret type, with redacted audit records. `crates/api/`, `crates/control-store/`, `crates/agent-protocol/` | Role and node scope, revision conflict, denial records and redaction. |
| `feat/sprint-18-safe-host-apply` · 5 | Validate allowed management paths, stage policy in a managed chain, use bounded rollback confirmation, and rotate a scoped secret with a documented recovery point. `crates/agent/`, `modules/security/`, `modules/secrets/`, `integration/security/` | Invalid rules, stale revision, apply timeout, rollback and restart during rotation. |
| `feat/sprint-18-security-ui` · 2 | Show pending/confirmed/rolled-back policy revisions and secret rotation status. Integrate audit filter and permission states. `frontend/` | Failed apply visibility, confirmation timeout, permission denial and no secret reveal. |
| `test/sprint-18-security-vm` · 3 | Disposable VM with separate management and service test clients, out-of-band recovery and synthetic secrets. `tests/e2e/security/`, `docs/qualification/security/` | Fixture reachability, redaction canary and teardown evidence. |

**Release gate.** Verify allowed and denied paths with real traffic and persistence after reboot. Trigger failure to confirm automatic rollback and preserve operator access. Verify the service accepts the rotated credential, rejects the former one after handover, and preserves service data. Record events and confirm logs/artifacts exclude credential material. Arbitrary firewall scripts, full SSH/WAF policy management, external KMS and cluster-wide secret rotation require separate slices.

## Sprint 19 · Assisted migration and cutover

**Demo.** Plan and rehearse a migration of one representative CloudPanel static site and database with a preflight report, isolated restore, controlled traffic switch and rehearsed rollback. Use [migration executor](../../integration/migration/) and the [MIG-02 qualification](../qualification/migration/MIG-02.md). [S18](sessions/S18.md) · [S35](sessions/S35.md) · [S53](sessions/S53.md) · [#19](https://github.com/scherenhaenden/FusionPanel/issues/19) · [#38](https://github.com/scherenhaenden/FusionPanel/issues/38) · [#56](https://github.com/scherenhaenden/FusionPanel/issues/56)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `feat/sprint-19-migration-preflight` · 3 | Read-only discovery for one confirmed CloudPanel export profile, version/capacity checks and explicit unsupported resource inventory. `modules/migration/` | Unsupported records, inconsistent exports, tenant mismatch and source preservation. |
| `feat/sprint-19-migration-control` · 5 | Persist an approved plan and progress, wire executor jobs, stage at an independent target and support controlled cutover/rollback with recorded checkpoints. `crates/api/`, `crates/control-store/`, `crates/agent-protocol/`, `crates/agent/`, `integration/migration/` | Stale plan, interrupted copy, mismatched digest, retry and explicit approval boundary. |
| `feat/sprint-19-migration-ui` · 3 | Show discovery, unsupported resources, dry-run results, user approvals, evidence and failed/rolled-back state. `frontend/` | No implicit execution, denied access and stable progress after reconnect. |
| `test/sprint-19-cutover-rollback-e2e` · 5 | Isolated source/target VM pair, synthetic export, test DNS and a runbook for write freeze, validation and rollback rehearsal. `tests/e2e/migration/`, `docs/qualification/migration/` | Fixture teardown, backup/restore integrity and recovery evidence recording. |

**Release gate.** Capture source baseline, restore independently, compare file hashes, database contents and tenant ownership, and observe test DNS/traffic switching. Interrupt the migration before promotion and prove source continuity. Rehearse rollback with a controlled write after cutover and verify the documented reconciliation or write-freeze guarantee. Record observed downtime, recovery time and recovery point for the fixture. Bulk moves, public customer DNS, mail data migration and zero downtime remain outside this lane. An unsupported write mode or missing rollback environment blocks the cutover gate.

## Sprint 20 · Bounded release candidate

**Demo.** An operator using the handbook alone can install, sign in, exercise the supported workflows, recover service after failure and independently verify the artifacts for the reviewed candidate. [S58](sessions/S58.md) · [S59](sessions/S59.md) · [S61](sessions/S61.md) · [S62](sessions/S62.md) · [S63](sessions/S63.md)

| Branch and effort | Deliverable and write paths | Focused verification |
| --- | --- | --- |
| `test/sprint-20-release-matrix` · 5 | Map explicitly supported modes to implemented fixture commands and proof requirements. Reuse CI/package checks and cover regressions across Sprints 1–19. `scripts/ci/`, `scripts/release/`, `docs/releases/`, and workflow changes assigned to the integration owner | Validate matrix paths, manifest consistency, changed-file selection and blocked-lane exit reporting. |
| `test/sprint-20-performance-budgets` · 3 | Define test workload, machine envelope and reviewed thresholds before collecting measurements. Measure service latency, queue recovery, backup/restore and resources under the supported load. `tests/qualification/performance/`, `docs/qualification/performance/` | Repeatable input manifest, bounded workloads and versioned measurement output. |
| `docs/sprint-20-operator-recovery` · 3 | Handbook for install, update, rollback, backup recovery, audit and supported workflows. Ship compatibility table and recovery checklist with actual evidence links. `docs/operations/`, `docs/releases/recovery/` | Independent operator rehearsal and link/command verification. |
| `fix/sprint-20-release-*` · 2 reserve | Fix defects found by the gate through focused PRs with separate paths and exact reproduction evidence. Assign issue and ownership per defect. | Run a regression case for each fix and requalify affected lanes after merge. |

**Release gate.** Start from clean checkout and build candidate twice with pinned inputs. Verify release metadata, SHA-256, clean install, supported upgrade/rollback, restore to an independent target and the supported workflow matrix against disposable real dependencies. Repeat the affected lanes on any new candidate and require the existing CI release and package checks. Missing environment, undecided performance limits or unresolved critical authorization, durability or recovery gaps block acceptance. Record unsupported variants, evidence status and compatibility in the release notes. Treat stable status, remote tagging and release publication as separate decisions under the [versioning policy](../engineering/VERSIONING.md).

## Decisions and fixture reservations

| Needed by | Decision or resource | Record at kickoff |
| --- | --- | --- |
| 11 | Mail daemon and signer versions, authoritative test zone and Debian VM | Pin versions/images and fixture identity, reserve VM capacity, confirm contained test mail and reboot evidence. |
| 12 | Supported upgrade pair and database schema compatibility | List old/new artifacts, exact backup and rollback guarantee, and the owner who can call rollback. |
| 13–15 | Docker/OCI host policy and the supported WordPress runtime | List images/digests, allowed workload options, runtime/network ownership and which app/version pair is covered. |
| 16 | Backup retention and restoration expectations | Record sample data and engine versions, protected generations, and target RTO/RPO if an SLA is required. |
| 17–18 | Two-node lab, transport trust and out-of-band access | Record VM topology, fault timeouts, node credential lifecycle, and firewall rollback access. |
| 19 | Legacy export format, write policy and cutover owner | Name supported version/profile, freeze or reconcile writes, define traffic rollback trigger, require source snapshot and restore proof. |
| 20 | Compatibility matrix, workload envelope and release audience | Agree supported versions and variants, performance thresholds, recovery instructions and exact acceptance run list. |

## Scope carried forward

PHP management remains deprioritized. Record deferred evidence for public mail delivery, DNSSEC, additional backup providers, catalog image scanning, HA/autoscaling, extra WordPress modes and wider migration variants in their existing work packages. Preserve current capability and mode IDs and carry forward the corresponding acceptance gaps. Any additional release claim needs its own evidence under the [feature preservation policy](../engineering/FEATURE_PRESERVATION.md).
